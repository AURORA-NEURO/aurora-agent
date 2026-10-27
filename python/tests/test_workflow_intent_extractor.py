"""Fixture boundary tests, not evidence of actual-model extraction completeness."""
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime, ProviderConfig, ProviderError, ProviderResponse
from prism_sdk.workflow_builder import WorkflowValidationError
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_intent_extractor import (
    LocalWorkflowIntentExtractor, WorkflowIntentExtractionError,
    MAX_INTENT_EXTRACTION_RESPONSE_BYTES,
)
from prism_sdk.workflow_intent_obligations import admit_workflow_intent_obligations, compile_workflow_intent_obligations


TASK = "🔬 Read metadata; preserve raw mode; archive the result."


def _operation(identity="fixture.read", *, description="Read public metadata", schema=None):
    return WorkflowOperation(identity, description, schema or {"type": "object", "properties": {
        "mode": {"const": "metadata"}, "payload": {}}, "additionalProperties": False},
        {"type": "object"}, "local", implementation="local")


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
