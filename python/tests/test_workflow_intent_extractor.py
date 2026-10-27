"""Fixture boundary tests, not evidence of actual-model extraction completeness."""
import hashlib
import json
from pathlib import Path
import socket

import pytest

from prism_sdk.authoring import canonical_json, content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime, ProviderConfig, ProviderError, ProviderResponse
from prism_sdk.workflow_builder import WorkflowValidationError
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_intent_extractor import (
    LocalWorkflowIntentExtractor, WorkflowIntentExtractionError,
    MAX_INTENT_EXTRACTION_RESPONSE_BYTES, validate_workflow_intent_extraction,
)
from prism_sdk.workflow_intent_obligations import admit_workflow_intent_obligations, compile_workflow_intent_obligations


TASK = "🔬 Read metadata; preserve raw mode; archive the result."


@pytest.fixture(autouse=True)
def _deny_accidental_network(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("intent extraction fixtures must not dispatch network requests")
    monkeypatch.setattr(socket, "socket", denied)
    monkeypatch.setattr(socket, "getaddrinfo", denied)


def _operation(identity="fixture.read", *, description="Read public metadata", schema=None, output_schema=None):
    return WorkflowOperation(identity, description, schema or {"type": "object", "properties": {
        "mode": {"const": "metadata"}, "payload": {}}, "additionalProperties": False},
        {"type": "object"} if output_schema is None else output_schema, "local", implementation="local")


def _catalog(count=1):
    catalog, sources = WorkflowOperationCatalog(), []
    def execute(arguments, **kwargs):
        sources.append(arguments)
        return WorkflowOperationResult({}, {})
    for index in range(count):
        catalog.register(_operation("fixture.read" if index == 0 else "fixture.read" + str(index)), execute)
    return catalog, sources


def _row(*, text="Read metadata", occurrence=0, **changes):
    return {"id": "read", "kind": "required_operation", "span": {"text": text, "occurrence": occurrence},
            "operation": "fixture.read", "output": True, **changes}


def _candidate(*rows):
    return {"action": "candidate", "obligations": list(rows or [_row()])}


def _extractor(response=None, *, catalog=None, callback=None, runtime=None, **options):
    catalog = catalog or _catalog()[0]
    runtime = runtime or LLMRuntime()
    requests = []
    def handler(request):
        requests.append(request)
        if callback:
            callback()
        if isinstance(response, BaseException):
            raise response
        if isinstance(response, ProviderResponse):
            return response
        return response if isinstance(response, str) else json.dumps(response or _candidate(), ensure_ascii=False)
    config = runtime.register_in_memory_provider("fixture", handler)
    extractor = LocalWorkflowIntentExtractor(catalog, runtime, model="fixture", provider="fixture", **options)
    return extractor, requests, config


def test_candidate_resolves_unicode_excerpt_exactly_and_never_becomes_accepted_or_executable():
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True)
    data = result.to_dict()
    assert result.status == "candidate" and len(requests) == 1 and sources == []
    assert data["span_resolution"][0]["span"] == {"start": 2, "end": 15, "text": "Read metadata"}
    assert data["obligations"]["interpretation_basis"] == "model_candidate"
    assert data["accepted"] is False and data["plan_created"] is False and data["source_dispatches"] == 0
    assert data["task_interpretation_completeness"] == "unassessed"
    assert admit_workflow_intent_obligations(TASK, result.obligations, catalog).status == "needs_clarification"
    claimed = data.pop("extraction_digest")
    assert claimed == content_digest(data)
    exported = result.to_dict()
    exported["obligations"]["interpretation_basis"] = "caller_supplied"
    assert result.obligations.to_dict()["interpretation_basis"] == "model_candidate"


def test_occurrence_selection_includes_overlapping_matches_without_changing_task_text():
    extractor, requests, _ = _extractor(_candidate(_row(text="aa", occurrence=1)))
    result = extractor.extract("aaa", approve_provider_call=True)
    assert result.obligations.to_dict()["obligations"][0]["span"] == {"start": 1, "end": 3, "text": "aa"}
    assert len(requests) == 1


@pytest.mark.parametrize("selector", [
    {"text": "read metadata", "occurrence": 0},
    {"text": "Read metadata ", "occurrence": 0},
    {"text": "Read metadata", "occurrence": 1},
    {"text": "Read metadata", "occurrence": True},
    {"text": "Read metadata", "occurrence": -1},
    {"text": "Read metadata", "occurrence": 0, "start": 2},
    {"text": " ", "occurrence": 0},
])
def test_invalid_exact_selector_is_retained_as_rejected_without_retry_or_excerpt_repair(selector):
    response = _candidate({**_row(), "span": selector})
    extractor, requests, _ = _extractor(response)
    result = extractor.extract(TASK, approve_provider_call=True)
    data = result.to_dict()
    assert result.status == "rejected" and result.obligations is None
    assert json.loads(data["raw_response"]["text"]) == response
    assert data["span_resolution"] == [] and data["obligations"] is None and len(requests) == 1


def test_unavailable_work_and_incompatible_essential_literal_survive_candidate_extraction():
    catalog, sources = _catalog()
    equality = {"id": "mode", "kind": "argument_equals", "span": {"text": "raw mode", "occurrence": 0},
                "operation": "fixture.read", "path": "/mode", "value": "raw"}
    unresolved = {"id": "archive", "kind": "unresolved_action", "span": {"text": "archive the result", "occurrence": 0},
                  "reason": "No archive writer in the catalogue"}
    extractor, requests, _ = _extractor(_candidate(_row(), equality, unresolved), catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True)
    rows = result.obligations.to_dict()["obligations"]
    assert rows[1]["value"] == "raw" and rows[2]["kind"] == "unresolved_action"
    # Explicitly constructing a separate caller assertion is not performed by extraction.
    declared = compile_workflow_intent_obligations(TASK, rows)
    receipt = admit_workflow_intent_obligations(TASK, declared, catalog).to_dict()
    assert {row["code"] for row in receipt["blocking_issues"]} == {"literal_schema_conflict", "unresolved_action"}
    assert receipt["requirements"] is None and sources == [] and len(requests) == 1


@pytest.mark.parametrize("literal", [True, 1, 1.0, {"items": [True, 1, 1.0]}])
def test_literal_types_remain_native_json_values_without_coercion(literal):
    row = {"id": "literal", "kind": "argument_equals", "operation": "fixture.read", "path": "/payload",
           "span": {"text": "preserve raw mode", "occurrence": 0}, "value": literal}
    extractor, _, _ = _extractor(_candidate(row))
    value = extractor.extract(TASK, approve_provider_call=True).obligations.to_dict()["obligations"][0]["value"]
    assert type(value) is type(literal) and content_digest(value) == content_digest(literal)


@pytest.mark.parametrize("changes", [
    {"interpretation_basis": "caller_supplied"}, {"accepted": True}, {"human_reviewed": True},
    {"kind": "required_operation", "output": 1}, {"id": "bad identity"},
    {"kind": "argument_equals", "path": "/bad~pointer", "value": "x"},
])
def test_model_authority_fields_and_invalid_native_rows_are_rejected(changes):
    extractor, requests, _ = _extractor(_candidate(_row(**changes)))
    result = extractor.extract(TASK, approve_provider_call=True)
    assert result.status == "rejected" and result.obligations is None and len(requests) == 1


def test_duplicate_keys_are_not_silently_resolved_by_runtime_json_parsing():
    text = json.dumps(_candidate()).replace('"output": true', '"output": false, "output": true')
    extractor, requests, _ = _extractor(text)
    result = extractor.extract(TASK, approve_provider_call=True)
    assert result.status == "rejected" and result.to_dict()["raw_response"]["text"] == text and len(requests) == 1


@pytest.mark.parametrize("action,status", [("clarify", "needs_clarification"), ("refuse", "refused")])
def test_explicit_terminal_response_is_not_a_candidate_or_plan(action, status):
    extractor, requests, _ = _extractor({"action": action, "message": "Please resolve the requested scope."})
    result = extractor.extract(TASK, approve_provider_call=True)
    assert result.status == status and result.obligations is None
    assert result.to_dict()["message"] == "Please resolve the requested scope." and len(requests) == 1


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_provider_approval_must_be_literal_true_before_any_invocation(approval):
    extractor, requests, _ = _extractor()
    with pytest.raises(WorkflowIntentExtractionError, match="approval"):
        extractor.extract(TASK, approve_provider_call=approval)
    assert requests == []


@pytest.mark.parametrize("selection", [[], ["fixture.read", "fixture.read"], [True], "fixture.read", ["absent.read"]])
def test_invalid_selection_refuses_without_silent_ranking_or_transport(selection):
    extractor, requests, _ = _extractor()
    with pytest.raises(ArgumentError):
        extractor.extract(TASK, approve_provider_call=True, operation_ids=selection)
    assert requests == []


def test_visibility_overflow_refuses_before_provider_instead_of_truncating_catalogue():
    catalog, sources = _catalog(3)
    extractor, requests, _ = _extractor(catalog=catalog, max_visible_operations=2)
    with pytest.raises(WorkflowIntentExtractionError, match="oversized"):
        extractor.extract(TASK, approve_provider_call=True)
    result = extractor.extract(TASK, approve_provider_call=True, operation_ids=["fixture.read"])
    assert result.to_dict()["visible_operations"] == ["fixture.read"] and len(requests) == 1 and sources == []


def test_oversized_context_refuses_before_inference_with_no_operation_summary_truncation():
    catalog, sources = _catalog()
    for index in range(12):
        catalog.register(_operation("large.read" + str(index), schema={"type": "object", "description": "x" * 180_000}),
                         lambda arguments, **kwargs: sources.append(arguments))
    extractor, requests, _ = _extractor(catalog=catalog)
    with pytest.raises(WorkflowIntentExtractionError, match="context"):
        extractor.extract(TASK, approve_provider_call=True)
    assert requests == [] and sources == []


def test_oversized_returned_text_retains_digest_and_size_without_a_truncated_candidate():
    text = json.dumps(_candidate()) + " " * MAX_INTENT_EXTRACTION_RESPONSE_BYTES
    extractor, requests, _ = _extractor(text)
    result = extractor.extract(TASK, approve_provider_call=True)
    raw = result.to_dict()["raw_response"]
    assert result.status == "rejected" and result.obligations is None
    assert raw == {"text": None, "text_digest": content_digest({"text": text}),
                   "utf8_bytes": len(text.encode()), "retention": "omitted_exceeds_bound"}
    assert len(requests) == 1


@pytest.mark.parametrize("response", [ProviderError("transport refused"), "not json"])
def test_transport_or_runtime_structured_failure_propagates_without_candidate_fallback(response):
    extractor, requests, _ = _extractor(response)
    with pytest.raises(ProviderError):
        extractor.extract(TASK, approve_provider_call=True)
    assert len(requests) == 1


def test_wrong_returned_model_is_rejected_by_runtime_before_candidate_creation():
    response = ProviderResponse("fixture", "other", json.dumps(_candidate()), 200, None, {}, {})
    extractor, requests, _ = _extractor(response)
    with pytest.raises(ProviderError, match="identity"):
        extractor.extract(TASK, approve_provider_call=True)
    assert len(requests) == 1


@pytest.mark.parametrize("kind", ["remote", "credential", "retry"])
def test_nonlocal_credentialed_or_retry_provider_refuses_before_invocation(kind):
    runtime = LLMRuntime()
    if kind == "retry":
        runtime.register_in_memory_provider("fixture", lambda request: pytest.fail("must not invoke"), max_attempts=2)
    else:
        runtime.register_provider(ProviderConfig(provider="fixture", base_url="https://example.org",
            requires_credential=kind == "credential", protocol="openai_chat_completions"))
    with pytest.raises(WorkflowIntentExtractionError):
        LocalWorkflowIntentExtractor(_catalog()[0], runtime, model="fixture", provider="fixture")


def test_callback_catalogue_drift_is_caught_before_concrete_transport():
    catalog, sources = _catalog()
    armed = False
    def clock():
        nonlocal armed
        if armed:
            armed = False
            catalog.register(_operation("changed.read"), lambda arguments, **kwargs: sources.append(arguments))
        return 1.0
    extractor, requests, _ = _extractor(catalog=catalog, runtime=LLMRuntime(clock=clock))
    armed = True
    with pytest.raises(WorkflowIntentExtractionError, match="changed"):
        extractor.extract(TASK, approve_provider_call=True)
    assert requests == [] and sources == []


def test_provider_configuration_drift_refuses_before_transport():
    extractor, requests, config = _extractor()
    object.__setattr__(config, "timeout_seconds", config.timeout_seconds + 1)
    with pytest.raises(WorkflowValidationError, match="changed"):
        extractor.extract(TASK, approve_provider_call=True)
    assert requests == []


def test_return_callback_catalogue_drift_cannot_publish_a_candidate():
    catalog, sources = _catalog()
    def mutate():
        catalog.register(_operation("changed.read"), lambda arguments, **kwargs: sources.append(arguments))
    extractor, requests, _ = _extractor(catalog=catalog, callback=mutate)
    with pytest.raises(WorkflowIntentExtractionError, match="changed"):
        extractor.extract(TASK, approve_provider_call=True)
    assert len(requests) == 1 and sources == []


def _restamp_extraction(data):
    data.pop("extraction_digest", None)
    data["extraction_digest"] = content_digest(data)
    return data


@pytest.mark.parametrize("response", [
    _candidate(), {"action": "clarify", "message": "Which scope?"},
    {"action": "refuse", "message": "Unsupported scope."},
    _candidate(_row(text="absent excerpt")),
    json.dumps(_candidate()) + " " * MAX_INTENT_EXTRACTION_RESPONSE_BYTES,
], ids=["candidate", "clarification", "refusal", "invalid-selector", "oversized-text"])
def test_offline_validation_replays_each_returned_outcome_without_new_provider_or_source_calls(response):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(response, catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True)
    checked = validate_workflow_intent_extraction(result.to_dict(), task=TASK, catalog=catalog)
    assert checked.to_dict() == result.to_dict() and len(requests) == 1 and sources == []


@pytest.mark.parametrize("tamper", [
    "raw_text", "native_rows", "span_resolution", "visible_operations", "response_schema_digest",
    "source_dispatches", "accepted", "provider_invocations", "unexpected_field", "status",
])
def test_restamping_extraction_digest_cannot_replace_raw_native_binding_or_claim_authority(tamper):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    data = extractor.extract(TASK, approve_provider_call=True).to_dict()
    if tamper == "raw_text":
        decision = json.loads(data["raw_response"]["text"])
        decision["obligations"][0]["output"] = False
        text = json.dumps(decision)
        data["raw_response"].update(text=text, text_digest=content_digest({"text": text}), utf8_bytes=len(text.encode()))
    elif tamper == "native_rows":
        rows = data["obligations"]["obligations"]
        rows[0]["output"] = False
        data["obligations"] = compile_workflow_intent_obligations(TASK, rows, interpretation_basis="model_candidate").to_dict()
    elif tamper == "span_resolution":
        data["span_resolution"][0]["span"]["start"] += 1
    else:
        data[tamper] = {"visible_operations": ["absent.read"], "response_schema_digest": "a" * 64,
            "source_dispatches": 1, "accepted": True, "provider_invocations": True,
            "unexpected_field": "approved", "status": "planned"}[tamper]
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=TASK, catalog=catalog)
    assert len(requests) == 1 and sources == []


def test_offline_validation_binds_exact_task_and_selected_operation_ids():
    catalog, sources = _catalog(2)
    extractor, requests, _ = _extractor(catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True, operation_ids=["fixture.read"])
    assert validate_workflow_intent_extraction(result, task=TASK, catalog=catalog,
        operation_ids=["fixture.read"]).status == "candidate"
    with pytest.raises(WorkflowIntentExtractionError):
        validate_workflow_intent_extraction(result, task=TASK + " ", catalog=catalog, operation_ids=["fixture.read"])
    with pytest.raises(WorkflowIntentExtractionError):
        validate_workflow_intent_extraction(result, task=TASK, catalog=catalog)
    with pytest.raises(WorkflowIntentExtractionError):
        validate_workflow_intent_extraction(result, task=TASK, operation_ids=["fixture.read"])
    assert len(requests) == 1 and sources == []


def test_default_protocol_0_2_exposes_complete_native_contracts_as_future_outputs_without_source_observations():
    catalog, sources = _catalog()
    output = {"type": "object", "required": ["schema", "items"], "additionalProperties": False,
              "properties": {"schema": {"const": "fixture-source-bundle/0.1"},
                             "items": {"type": "array", "items": {"type": "object", "properties": {
                                 "score": {"enum": [1, 1.0, True]}, "native": {"type": ["object", "null"]}}}}}}
    catalog.register(_operation("fixture.output", output_schema=output),
                     lambda arguments, **kwargs: sources.append(arguments))
    extractor, requests, _ = _extractor(catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True)
    context = json.loads(requests[0].messages[0]["content"])
    assert context["protocol"] == result.to_dict()["protocol"] == "aurora-workflow-intent-extraction-protocol/0.2"
    assert context["operations"] == [operation.to_dict() for operation in catalog.operations()]
    assert all(set(row) == {"schema", "operation_id", "description", "input_schema", "output_schema",
                           "integration_id", "auth_mode", "read_only", "implementation"}
               for row in context["operations"])
    returned_schema = next(row["output_schema"] for row in context["operations"] if row["operation_id"] == "fixture.output")
    assert returned_schema == output
    assert [type(value) for value in returned_schema["properties"]["items"]["items"]["properties"]["score"]["enum"]] == [int, float, bool]
    assert any("possible future returned value" in rule and "not a source result" in rule for rule in context["rules"])
    assert any("whole future returned value" in rule and "input flag" in rule for rule in context["rules"])
    assert any("not execution grants" in rule for rule in context["rules"])
    assert "source_results" not in context and "plan" not in context
    assert result.to_dict()["accepted"] is False and result.to_dict()["plan_created"] is False
    assert result.obligations.to_dict()["interpretation_basis"] == "model_candidate"
    assert sources == [] and len(requests) == 1


def test_future_work_role_additions_change_only_two_generic_rules_without_changing_response_or_call_boundaries():
    from prism_sdk.workflow_intent_extractor import _request_value
    catalog, sources = _catalog()
    operations = catalog.operations()
    previous = _request_value(TASK, catalog.catalog_digest, "fixture", operations,
                              "aurora-workflow-intent-extraction-protocol/0.2")
    current = _request_value(TASK, catalog.catalog_digest, "fixture", operations,
                             "aurora-workflow-intent-extraction-protocol/0.3")
    before, after = (json.loads(request["messages"][0]["content"]) for request in (previous, current))
    assert after["rules"][:2] == [
        "Interpret the task as mandatory declarations for a future workflow run by a separate executor. This turn does not execute sources or build a plan.",
        "Your lack of callable tools, network access or credentials in this turn does not by itself make a catalogue-defined future action unsupported. Match requested work to declared contracts; never infer live reachability, authentication state, execution grants or scientific validity from catalogue labels.",
    ]
    assert after["rules"][2:] == before["rules"]
    assert {key: value for key, value in after.items() if key not in {"protocol", "rules"}} == {
        key: value for key, value in before.items() if key not in {"protocol", "rules"}}
    assert current["messages"][0]["role"] == previous["messages"][0]["role"] == "system"
    assert len(current["messages"]) == len(previous["messages"]) == 1
    assert {key: value for key, value in current.items() if key != "messages"} == {
        key: value for key, value in previous.items() if key != "messages"}
    assert len(canonical_json(current).encode("utf-8")) - len(canonical_json(previous).encode("utf-8")) < 600
    assert any("including unavailable capabilities" in rule for rule in after["rules"])
    assert any("even when they conflict" in rule for rule in after["rules"])
    assert any("Use clarify or refuse" in rule for rule in after["rules"])
    assert sources == []


def test_available_future_read_fixture_preserves_input_literals_and_output_but_remains_unreviewed():
    task = "Read public metadata in metadata mode with page size 4 and keep the returned value."
    catalog, sources = WorkflowOperationCatalog(), []
    operation = WorkflowOperation("fixture.future_read", "Read public metadata", {
        "type": "object", "properties": {"mode": {"const": "metadata"},
            "page_size": {"type": "integer", "minimum": 1, "maximum": 10}},
        "required": ["mode", "page_size"], "additionalProperties": False},
        {"type": "object", "properties": {"records": {"type": "array", "items": {"type": "object"}}},
         "required": ["records"]}, "fixture", auth_mode="none", implementation="live_adapter")
    catalog.register(operation, lambda arguments, **kwargs: sources.append(arguments))
    response = _candidate(
        _row(text="Read public metadata", operation=operation.operation_id),
        {"id": "mode", "kind": "argument_equals", "operation": operation.operation_id,
         "path": "/mode", "value": "metadata", "span": {"text": "metadata mode", "occurrence": 0}},
        {"id": "size", "kind": "argument_equals", "operation": operation.operation_id,
         "path": "/page_size", "value": 4, "span": {"text": "page size 4", "occurrence": 0}})
    extractor, requests, _ = _extractor(response, catalog=catalog)
    result = extractor.extract(task, approve_provider_call=True)
    checked = validate_workflow_intent_extraction(result, task=task, catalog=catalog)
    rows = checked.obligations.to_dict()["obligations"]
    assert rows[0]["operation"] == operation.operation_id and rows[0]["output"] is True
    assert [(row["path"], row["value"]) for row in rows[1:]] == [("/mode", "metadata"), ("/page_size", 4)]
    assert type(rows[2]["value"]) is int
    admission = admit_workflow_intent_obligations(task, checked.obligations, catalog).to_dict()
    assert admission["status"] == "needs_clarification" and admission["requirements"] is None
    assert admission["blocking_issues"] == [{"code": "unreviewed_model_candidate"}]
    assert checked.to_dict()["accepted"] is False and checked.to_dict()["plan_created"] is False
    assert sources == [] and len(requests) == 1


@pytest.mark.parametrize("row,expected_code", [
    ({"id": "archive", "kind": "unresolved_action", "span": {"text": "archive the result", "occurrence": 0},
      "reason": "The catalogue has no archive writer; reading does not perform the essential write."}, "unresolved_action"),
    ({"id": "mode", "kind": "argument_equals", "span": {"text": "raw mode", "occurrence": 0},
      "operation": "fixture.read", "path": "/mode", "value": "raw"}, "literal_schema_conflict"),
], ids=["unsupported-essential-write", "incompatible-exact-const"])
def test_default_extraction_does_not_remove_unavailable_work_or_replace_conflicting_literals(row, expected_code):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(_candidate(_row(), row), catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True)
    rows = result.obligations.to_dict()["obligations"]
    assert canonical_json({key: value for key, value in rows[1].items() if key != "span"}) == canonical_json({
        key: value for key, value in row.items() if key != "span"})
    receipt = admit_workflow_intent_obligations(TASK, result.obligations, catalog).to_dict()
    assert {issue["code"] for issue in receipt["blocking_issues"]} == {"unreviewed_model_candidate", expected_code}
    assert receipt["requirements"] is None and receipt["status"] != "ready_for_constrained_authoring"
    assert sources == [] and len(requests) == 1


def test_explicit_selection_exposes_only_selected_complete_descriptors_without_summary_or_renaming():
    catalog, sources = _catalog(2)
    extractor, requests, _ = _extractor(catalog=catalog)
    result = extractor.extract(TASK, approve_provider_call=True, operation_ids=["fixture.read1"])
    context = json.loads(requests[0].messages[0]["content"])
    assert context["operations"] == [catalog.resolve("fixture.read1").to_dict()]
    assert result.to_dict()["visible_operations"] == ["fixture.read1"]
    assert validate_workflow_intent_extraction(result, task=TASK, catalog=catalog,
        operation_ids=["fixture.read1"]).to_dict() == result.to_dict()
    assert sources == [] and len(requests) == 1


def test_output_contract_alone_can_exceed_context_bound_and_refuses_before_provider_or_executor():
    catalog, sources = _catalog()
    for index in range(3):
        catalog.register(_operation("large.output" + str(index), output_schema={
            "type": "object", "description": "x" * 750_000}),
            lambda arguments, **kwargs: sources.append(arguments))
    before = catalog.to_dict()
    extractor, requests, _ = _extractor(catalog=catalog)
    with pytest.raises(WorkflowIntentExtractionError, match="context"):
        extractor.extract(TASK, approve_provider_call=True)
    assert requests == [] and sources == [] and catalog.to_dict() == before


def _legacy_fixture():
    return json.loads((Path(__file__).parent / "fixtures/workflow_intent_extraction_protocol_0_1.json").read_text(encoding="utf-8"))


def _full_contract_fixture():
    return json.loads((Path(__file__).parent / "fixtures/workflow_intent_extraction_protocol_0_2.json").read_text(encoding="utf-8"))


def _full_contract_fixture_catalog():
    catalog = WorkflowOperationCatalog()
    for descriptor in _full_contract_fixture()["operation_descriptors"]:
        operation = WorkflowOperation(**{key: value for key, value in descriptor.items() if key != "schema"})
        catalog.register(operation, lambda *_args, **_kwargs: pytest.fail("legacy fixture invoked a source"))
    return catalog


def _actual_future_role_terminal_fixture():
    return json.loads((Path(__file__).parent / "fixtures/workflow_intent_extraction_protocol_0_3_actual_terminal.json").read_text(encoding="utf-8"))


def _actual_future_role_fixture_catalog():
    catalog = WorkflowOperationCatalog()
    for descriptor in _actual_future_role_terminal_fixture()["operation_descriptors"]:
        operation = WorkflowOperation(**{key: value for key, value in descriptor.items() if key != "schema"})
        catalog.register(operation, lambda *_args, **_kwargs: pytest.fail("actual terminal replay invoked a source"))
    return catalog


def test_actual_protocol_0_3_acknowledgement_does_not_become_declarations_after_default_revert(monkeypatch):
    from prism_sdk.workflow_intent_extractor import WORKFLOW_INTENT_EXTRACTION_PROTOCOL, _request_value
    fixture, catalog = _actual_future_role_terminal_fixture(), _actual_future_role_fixture_catalog()
    monkeypatch.setattr(LLMRuntime, "invoke", lambda *_args, **_kwargs: pytest.fail("actual terminal replay invoked a model"))
    assert WORKFLOW_INTENT_EXTRACTION_PROTOCOL == "aurora-workflow-intent-extraction-protocol/0.2"
    assert fixture["captured_source_head"] == "25c6816b4012c7c4152c2e62393e955bc0883a82"
    assert fixture["catalog_digest"] == catalog.catalog_digest
    original = fixture["extraction"]
    request = _request_value(fixture["task"], catalog.catalog_digest, original["model"], catalog.operations(), original["protocol"])
    assert canonical_json(request) == canonical_json(fixture["request_value"])
    assert content_digest(request) == original["request_digest"]
    assert len(canonical_json(request).encode("utf-8")) == original["context_bytes_including_schema"]
    checked = validate_workflow_intent_extraction(original, task=fixture["task"], catalog=catalog).to_dict()
    assert canonical_json(checked) == canonical_json(original)
    assert checked["protocol"] == "aurora-workflow-intent-extraction-protocol/0.3"
    assert checked["status"] == "needs_clarification" and checked["obligations"] is None and checked["span_resolution"] == []
    assert checked["message"] == "I will execute both operations with the exact parameters specified. Each operation's complete future returned value will be provided as a separate final output."
    assert checked["accepted"] is False and checked["plan_created"] is False and checked["source_dispatches"] == 0
    assert checked["task_interpretation_completeness"] == "unassessed"


@pytest.mark.parametrize("protocol", ["aurora-workflow-intent-extraction-protocol/0.1",
                                     "aurora-workflow-intent-extraction-protocol/0.2"])
def test_actual_protocol_0_3_terminal_cannot_be_restamped_as_an_older_request(protocol):
    fixture, catalog = _actual_future_role_terminal_fixture(), _actual_future_role_fixture_catalog()
    data = fixture["extraction"]
    data["protocol"] = protocol
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=fixture["task"], catalog=catalog)


@pytest.mark.parametrize("case_index", [0, 1], ids=["candidate", "refusal"])
def test_actual_protocol_0_2_producer_fixture_keeps_exact_request_bytes_and_unreviewed_status(monkeypatch, case_index):
    from prism_sdk.workflow_intent_extractor import _request_value
    fixture, catalog = _full_contract_fixture(), _full_contract_fixture_catalog()
    record = fixture["cases"][case_index]
    monkeypatch.setattr(LLMRuntime, "invoke", lambda *_args, **_kwargs: pytest.fail("legacy replay invoked a model"))
    assert fixture["captured_source_head"] == "12a18b8348826749669ec72fdd67a916602dc521"
    legacy_path = Path(__file__).parent / "fixtures/workflow_intent_extraction_protocol_0_1.json"
    assert hashlib.sha256(legacy_path.read_bytes()).hexdigest() == fixture["preserved_protocol_0_1_fixture_sha256"]
    assert fixture["catalog_digest"] == catalog.catalog_digest
    request = _request_value(fixture["task"], catalog.catalog_digest, "fixture", catalog.operations(),
                             "aurora-workflow-intent-extraction-protocol/0.2")
    assert canonical_json(request) == canonical_json(record["request_value"])
    assert content_digest(request) == record["extraction"]["request_digest"]
    assert len(canonical_json(request).encode("utf-8")) == record["extraction"]["context_bytes_including_schema"]
    checked = validate_workflow_intent_extraction(record["extraction"], task=fixture["task"], catalog=catalog)
    assert canonical_json(checked.to_dict()) == canonical_json(record["extraction"])
    assert checked.to_dict()["protocol"] == "aurora-workflow-intent-extraction-protocol/0.2"
    assert checked.to_dict()["accepted"] is False and checked.to_dict()["plan_created"] is False
    if checked.status == "candidate":
        assert admit_workflow_intent_obligations(fixture["task"], checked.obligations, catalog).status == "needs_clarification"


@pytest.mark.parametrize("case_index", [0, 1], ids=["candidate", "refusal"])
def test_actual_legacy_producer_fixture_replays_without_promoting_protocol_or_claiming_output_exposure(tmp_path, monkeypatch, case_index):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker
    fixture = _legacy_fixture()
    record = fixture["cases"][case_index]
    assert fixture["captured_source_head"] == "dba24ef86d54d4ac12af8d87dcf92cf7dc32607e"
    monkeypatch.setattr(LLMRuntime, "invoke", lambda *_args, **_kwargs: pytest.fail("legacy replay invoked a model"))
    monkeypatch.setattr(WorkflowOperationCatalog, "execute", lambda *_args, **_kwargs: pytest.fail("legacy replay invoked a source"))
    worker = WorkflowAPIWorker(tmp_path)
    assert fixture["catalog_digest"] == worker.catalog.catalog_digest
    assert fixture["operation_descriptors"] == [operation.to_dict() for operation in worker.catalog.operations()]
    context = json.loads(record["request_value"]["messages"][0]["content"])
    assert all(set(row) == {"operation_id", "description", "input_schema", "read_only"} for row in context["operations"])
    assert record["extraction"]["request_digest"] == content_digest(record["request_value"])
    checked = validate_workflow_intent_extraction(record["extraction"], task=fixture["task"], catalog=worker.catalog)
    assert checked.to_dict() == record["extraction"]
    assert checked.to_dict()["protocol"] == "aurora-workflow-intent-extraction-protocol/0.1"
    assert checked.to_dict()["accepted"] is False


def test_persisted_legacy_candidate_review_reloads_and_accepts_separately_without_rewriting_original_bytes(tmp_path, monkeypatch):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker, WorkflowAPIError
    fixture = _legacy_fixture()
    extraction = fixture["cases"][0]["extraction"]
    monkeypatch.setattr(LLMRuntime, "invoke", lambda *_args, **_kwargs: pytest.fail("legacy review loading invoked a model"))
    monkeypatch.setattr(WorkflowOperationCatalog, "execute", lambda *_args, **_kwargs: pytest.fail("legacy review loading invoked a source"))
    worker = WorkflowAPIWorker(tmp_path)
    request = {"task": fixture["task"], "planner": "local-model", "max_steps": 1}
    saved = worker._save_review(worker._review_context(request), extraction["obligations"],
        {"kind": "local_model_candidate", "extraction": extraction})
    path = tmp_path / "reviews" / saved["review_id"] / "review.json"
    original = path.read_bytes()
    restarted = WorkflowAPIWorker(tmp_path)
    loaded = restarted.handle({"action": "get_review", "review_id": saved["review_id"]})
    assert loaded == saved and loaded["acceptance"] is None
    assert loaded["origin"]["extraction"] == extraction
    accepted = restarted.handle({"action": "accept_review", "review_id": saved["review_id"],
        "expected_review_digest": saved["review_digest"], "accept_caller_declarations": True})
    assert accepted["acceptance"]["admission"]["status"] == "ready_for_constrained_authoring"
    assert path.read_bytes() == original
    with pytest.raises(WorkflowAPIError, match="approval"):
        restarted.handle({**request, "action": "plan", "intent_review_id": saved["review_id"],
            "expected_acceptance_digest": accepted["acceptance"]["acceptance_digest"]})
    assert not list((tmp_path / "plans").iterdir()) and not list((tmp_path / "runs").iterdir())


@pytest.mark.parametrize("protocol", ["aurora-workflow-intent-extraction-protocol/0.1",
    "aurora-workflow-intent-extraction-protocol/0.3", "unknown", None])
def test_new_request_cannot_be_relabelled_as_another_protocol_by_restamping_when_catalogue_is_supplied(protocol):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    data = extractor.extract(TASK, approve_provider_call=True).to_dict()
    data["protocol"] = protocol
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=TASK, catalog=catalog)
    assert len(requests) == 1 and sources == []


@pytest.mark.parametrize("protocol", ["aurora-workflow-intent-extraction-protocol/0.2",
                                     "aurora-workflow-intent-extraction-protocol/0.3"])
def test_legacy_request_cannot_be_relabelled_as_full_output_contract_exposure(tmp_path, protocol):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker
    fixture = _legacy_fixture()
    data = json.loads(json.dumps(fixture["cases"][0]["extraction"]))
    data["protocol"] = protocol
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=fixture["task"], catalog=WorkflowAPIWorker(tmp_path).catalog)


@pytest.mark.parametrize("protocol", ["aurora-workflow-intent-extraction-protocol/0.1",
                                     "aurora-workflow-intent-extraction-protocol/0.3"])
def test_protocol_0_2_request_cannot_be_downgraded_or_claim_the_new_future_executor_rules(protocol):
    fixture, catalog = _full_contract_fixture(), _full_contract_fixture_catalog()
    data = fixture["cases"][0]["extraction"]
    data["protocol"] = protocol
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=fixture["task"], catalog=catalog)


@pytest.mark.parametrize("field,value", [("request_digest", "a" * 64), ("context_bytes_including_schema", 1)])
def test_versioned_request_binding_rejects_restamped_digest_or_context_length_drift(field, value):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    data = extractor.extract(TASK, approve_provider_call=True).to_dict()
    data[field] = value
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=TASK, catalog=catalog)
    assert len(requests) == 1 and sources == []
