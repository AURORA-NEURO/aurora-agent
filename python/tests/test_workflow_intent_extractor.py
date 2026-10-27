"""Fixture boundary tests, not evidence of actual-model extraction completeness."""
import json
from pathlib import Path
import socket

import pytest

from prism_sdk.authoring import content_digest
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


def test_protocol_0_2_exposes_complete_native_contracts_as_future_outputs_without_source_observations():
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


@pytest.mark.parametrize("protocol", ["aurora-workflow-intent-extraction-protocol/0.1", "unknown", None])
def test_new_request_cannot_be_relabelled_as_another_protocol_by_restamping_when_catalogue_is_supplied(protocol):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    data = extractor.extract(TASK, approve_provider_call=True).to_dict()
    data["protocol"] = protocol
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=TASK, catalog=catalog)
    assert len(requests) == 1 and sources == []


def test_legacy_request_cannot_be_relabelled_as_full_output_contract_exposure(tmp_path):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker
    fixture = _legacy_fixture()
    data = json.loads(json.dumps(fixture["cases"][0]["extraction"]))
    data["protocol"] = "aurora-workflow-intent-extraction-protocol/0.2"
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=fixture["task"], catalog=WorkflowAPIWorker(tmp_path).catalog)


@pytest.mark.parametrize("field,value", [("request_digest", "a" * 64), ("context_bytes_including_schema", 1)])
def test_versioned_request_binding_rejects_restamped_digest_or_context_length_drift(field, value):
    catalog, sources = _catalog()
    extractor, requests, _ = _extractor(catalog=catalog)
    data = extractor.extract(TASK, approve_provider_call=True).to_dict()
    data[field] = value
    with pytest.raises(WorkflowIntentExtractionError, match="retained extraction"):
        validate_workflow_intent_extraction(_restamp_extraction(data), task=TASK, catalog=catalog)
    assert len(requests) == 1 and sources == []
