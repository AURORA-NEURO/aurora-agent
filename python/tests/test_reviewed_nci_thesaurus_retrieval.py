"""Fixed ontology scope, byte replay, unknown completeness and refusal invariants."""
import base64
from copy import deepcopy
import hashlib
import json
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.reviewed_public_http import ReviewedPublicHttpError
from prism_sdk.reviewed_nci_thesaurus_retrieval import (
    NciThesaurusHttpResponse, NciThesaurusRetrievalConfig, NciThesaurusRetrievalError,
    register_nci_thesaurus_operations, retrieve_nci_thesaurus_glioma, validate_nci_thesaurus_bundle,
)
from prism_sdk.workflow_integrations import WorkflowOperationCatalog

CLOCK = "2026-10-27T00:00:00Z"
URL = ("https://api-evsrest.nci.nih.gov/api/v1/concept/ncit/C3059"
    "?include=definitions,children,parents,roles,associations&limit=100")


def related(code="C123", name="Fixture child"):
    return {"code": code, "name": name, "terminology": "ncit", "version": "26.10d", "active": True, "leaf": False}


def concept():
    # Synthetic native-shape fixtures do not claim to be a captured NCI response.
    return {"code": "C3059", "name": "Glioma", "terminology": "ncit", "version": "26.10d",
        "active": True, "leaf": False, "conceptStatus": "DEFAULT",
        "definitions": [{"definition": "Fixture definition, not a source quotation.", "type": "DEFINITION",
            "source": "NCI", "qualifiers": [{"type": "Attribution", "value": "Fixture attribution"}]}],
        "children": [related()], "parents": [related("C124", "Fixture parent")],
        "roles": [{"code": "R1", "type": "Fixture_Role", "relatedCode": "C125", "relatedName": "Fixture related concept",
            "source": "NCI", "group": "0", "qualifiers": [{"code": "Q1", "type": "Fixture_qualifier", "value": "Fixture value"}]}],
        "associations": [{"code": "A1", "type": "Fixture_Association", "relatedCode": "C126", "relatedName": "Fixture association target", "source": "NCI"}]}


class Fixture:
    def __init__(self, value=None, *, body=None):
        self.value = concept() if value is None else value
        self.body, self.calls = body, []

    def __call__(self, url, *, http_config):
        self.calls.append((url, http_config))
        body = json.dumps(self.value).encode() if self.body is None else self.body
        return NciThesaurusHttpResponse(200, body, url, len(body), hashlib.sha256(body).hexdigest(),
            http_config.transport_policy_digest, http_config.config_digest, "application/json")


def read(fixture=None):
    return retrieve_nci_thesaurus_glioma(approve_source_dispatch=True, retrieved_at=CLOCK,
        fetcher=Fixture() if fixture is None else fixture)


def test_one_fixed_route_retains_exact_raw_bytes_native_source_qualifiers_and_relation_labels():
    body = ("  " + json.dumps(concept(), indent=1) + "\n").encode()
    fixture = Fixture(body=body)
    bundle = read(fixture)
    assert len(fixture.calls) == 1 and fixture.calls[0][0] == URL
    assert fixture.calls[0][1].allowed_hosts == ("api-evsrest.nci.nih.gov",)
    assert fixture.calls[0][1].max_response_bytes == 1_000_000
    source = bundle["source_inputs"][0]
    assert base64.b64decode(source["response_body_base64"], validate=True) == body
    assert source["response_digest"] == hashlib.sha256(body).hexdigest()
    assert source["native_payload"] == concept()
    assert bundle["concept"]["preferred_name"] == "Glioma"
    assert bundle["concept"]["definitions"][0]["qualifiers"][0]["value"] == "Fixture attribution"
    assert [row["source_relationship_label"] for row in bundle["relationships"]] == ["children", "parents", "Fixture_Role", "Fixture_Association"]
    assert bundle["relationships"][2]["native_record"]["group"] == "0"
    assert bundle["relationships"][2]["source_pointer"] == "/roles/0"
    assert bundle["coverage"]["exhaustiveness"] == "not_independently_established"
    assert bundle["coverage"]["clinical_or_cohort_evidence"] is False
    assert bundle["receipts"][0]["source_request_count"] == 1
    assert bundle["transport"]["mode"] == "caller_supplied"
    assert validate_nci_thesaurus_bundle(bundle) == bundle


def test_unrequested_parts_have_unknown_omitted_counts_and_unknown_influence_without_sufficiency_claim():
    bundle = read()
    coverage = bundle["coverage"]
    assert {row["source_part"] for row in coverage["omissions"]} == set(coverage["not_requested_concept_parts"])
    assert all(row["reason"] == "not_requested" and row["omitted_count"] is None
        and row["influence"] == "unknown" for row in coverage["omissions"])
    assert coverage["exhaustiveness"] == "not_independently_established"
    assert "sufficient" not in coverage
    assert validate_nci_thesaurus_bundle(bundle) == bundle


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_source_approval_is_required_before_any_callback(approval):
    fixture = Fixture()
    with pytest.raises(NciThesaurusRetrievalError, match="approval_required"):
        retrieve_nci_thesaurus_glioma(approve_source_dispatch=approval, fetcher=fixture)
    assert not fixture.calls


@pytest.mark.parametrize("part", ["definitions", "children", "parents", "roles", "associations"])
def test_missing_null_and_explicit_empty_source_parts_remain_distinct(part):
    missing = concept()
    del missing[part]
    null = {**concept(), part: None}
    empty = {**concept(), part: []}
    states = [read(Fixture(value))["coverage"]["concept_parts"][part] for value in (missing, null, empty)]
    assert states == [
        {"state": "not_reported", "returned_count": None, "field_presence": "absent"},
        {"state": "not_reported", "returned_count": None, "field_presence": "null"},
        {"state": "empty_success", "returned_count": 0, "field_presence": "array"}]


@pytest.mark.parametrize("key,value", [
    ("code", "C3058"), ("name", "glioma"), ("terminology", "ncim"), ("version", None),
    ("version", ""), ("version", "bad/version"), ("leaf", 0), ("active", "true"),
    ("definitions", {}), ("children", "[]"), ("unknownField", 1), ("fromRecord", 0),
    ("pageSize", 100), ("total", 1), ("next", None), ("ct", 100),
])
def test_identity_schema_or_unknown_paging_drift_refuses_whole_response(key, value):
    payload = concept()
    payload[key] = value
    with pytest.raises(NciThesaurusRetrievalError):
        read(Fixture(payload))


@pytest.mark.parametrize("part", ["definitions", "children", "parents", "roles", "associations"])
def test_documented_ct_limit_placeholders_never_become_complete_or_empty_results(part):
    payload = concept()
    payload[part].append({"ct": 101})
    with pytest.raises(NciThesaurusRetrievalError, match="limit_marker"):
        read(Fixture(payload))


@pytest.mark.parametrize("change", ["same_code", "version_drift", "missing_version", "wrong_terminology", "duplicate", "both_directions", "unknown_child_parts"])
def test_ambiguous_or_inconsistent_related_concepts_refuse(change):
    payload = concept()
    child = payload["children"][0]
    if change == "same_code":
        child["code"] = "C3059"
    elif change == "version_drift":
        child["version"] = "26.09d"
    elif change == "missing_version":
        del child["version"]
    elif change == "wrong_terminology":
        child["terminology"] = "ncim"
    elif change == "duplicate":
        payload["children"].append(deepcopy(child))
    elif change == "both_directions":
        payload["parents"].append(deepcopy(child))
    else:
        child["children"] = []
    with pytest.raises(NciThesaurusRetrievalError):
        read(Fixture(payload))


@pytest.mark.parametrize("change", ["definition_missing_text", "invalid_definition", "qualifier_missing_value", "qualifier_ct",
    "unknown_role", "related_code", "role_type", "role_group", "duplicate_role", "association_group"])
def test_malformed_definition_qualifier_or_relation_is_not_silently_dropped(change):
    payload = concept()
    if change == "definition_missing_text":
        del payload["definitions"][0]["definition"]
    elif change == "invalid_definition":
        payload["definitions"][0]["definition"] = ""
    elif change == "qualifier_missing_value":
        del payload["definitions"][0]["qualifiers"][0]["value"]
    elif change == "qualifier_ct":
        payload["roles"][0]["qualifiers"].append({"ct": 100})
    elif change == "unknown_role":
        payload["roles"][0]["inventedFlag"] = True
    elif change == "related_code":
        payload["roles"][0]["relatedCode"] = "arbitrary"
    elif change == "role_type":
        payload["roles"][0]["type"] = None
    elif change == "role_group":
        payload["roles"][0]["group"] = 1
    elif change == "duplicate_role":
        payload["roles"].append(deepcopy(payload["roles"][0]))
    else:
        payload["associations"][0]["group"] = "1"
    with pytest.raises(NciThesaurusRetrievalError):
        read(Fixture(payload))


def test_source_omits_definition_attribution_without_inventing_a_source():
    payload = concept()
    del payload["definitions"][0]["source"]
    bundle = read(Fixture(payload))
    assert "source" not in bundle["concept"]["definitions"][0]
    assert validate_nci_thesaurus_bundle(bundle) == bundle


def test_part_qualifier_string_and_response_bounds_refuse_without_truncation():
    values = []
    value = concept()
    value["children"] = [related(f"C{1000+i}") for i in range(101)]
    values.append(value)
    value = concept()
    value["roles"][0]["qualifiers"] *= 33
    values.append(value)
    value = concept()
    value["definitions"][0]["definition"] = "x"*16_385
    values.append(value)
    for value in values:
        with pytest.raises(NciThesaurusRetrievalError):
            read(Fixture(value))
    with pytest.raises(NciThesaurusRetrievalError, match="byte_bound"):
        read(Fixture(body=b" "*1_000_001))


@pytest.mark.parametrize("body", [b'[]', b'null', b'{"code":"C3059","code":"C1"}', b'{"x":NaN}', b'\xff', b'', b'{} trailing'])
def test_duplicate_nonfinite_nonobject_or_malformed_json_never_becomes_empty_success(body):
    with pytest.raises((NciThesaurusRetrievalError, ReviewedPublicHttpError)):
        read(Fixture(body=body))


@pytest.mark.parametrize("field", ["concept", "relationships", "coverage", "receipts", "query", "limitations", "transport", "source_inputs"])
def test_restamped_derived_or_native_tampering_fails_exact_byte_replay(field):
    value = read()
    if field == "source_inputs":
        value[field][0]["native_payload"]["definitions"] = []
        value[field][0]["payload_digest"] = content_digest(value[field][0]["native_payload"])
    elif isinstance(value[field], list):
        value[field] = []
    else:
        value[field]["tampered"] = True
    value["bundle_digest"] = content_digest({key: child for key, child in value.items() if key != "bundle_digest"})
    with pytest.raises(NciThesaurusRetrievalError):
        validate_nci_thesaurus_bundle(value)


def test_retained_raw_bytes_and_receipt_cannot_disagree_even_after_bundle_restamp():
    value = read()
    source = value["source_inputs"][0]
    source["response_body_base64"] = base64.b64encode(json.dumps(concept(), indent=2).encode()).decode()
    value["bundle_digest"] = content_digest({key: child for key, child in value.items() if key != "bundle_digest"})
    with pytest.raises(NciThesaurusRetrievalError, match="does_not_replay"):
        validate_nci_thesaurus_bundle(value)


@pytest.mark.parametrize("field,value", [("response_body_base64", "not base64!"), ("response_body_base64", 1)])
def test_invalid_retained_bytes_refuse_a_typed_error(field, value):
    bundle = read()
    bundle["source_inputs"][0][field] = value
    with pytest.raises(NciThesaurusRetrievalError):
        validate_nci_thesaurus_bundle(bundle)


def test_typed_http_rate_limit_preserves_failure_metadata_and_never_retries():
    calls = []
    def fail(url, **_kwargs):
        calls.append(url)
        raise ReviewedPublicHttpError("http_status_not_success", status_code=429, retry_after_seconds=5)
    with pytest.raises(ReviewedPublicHttpError) as error:
        retrieve_nci_thesaurus_glioma(fetcher=fail, approve_source_dispatch=True)
    assert calls == [URL] and error.value.status_code == 429
    assert error.value.retry_after_seconds == 5 and error.value.automatic_retry is False


def test_application_failure_is_sanitized_without_success_or_automatic_retry():
    calls = []
    def fail(url, **_kwargs):
        calls.append(url)
        raise RuntimeError("private callback text")
    with pytest.raises(NciThesaurusRetrievalError, match="source_request_failed") as error:
        retrieve_nci_thesaurus_glioma(fetcher=fail, approve_source_dispatch=True)
    assert calls == [URL] and error.value.__suppress_context__ is True
    assert "private" not in str(error.value)


def test_elapsed_application_deadline_cannot_become_success():
    with patch("prism_sdk.reviewed_nci_thesaurus_retrieval.monotonic", side_effect=[10.0, 41.0]):
        with pytest.raises(NciThesaurusRetrievalError, match="elapsed_deadline"):
            read()


@pytest.mark.parametrize("field,value", [("body", b'{}'), ("response_bytes", 0), ("config_digest", "0"*64),
    ("transport_policy_digest", "0"*64), ("request_url", "https://api-evsrest.nci.nih.gov/unreviewed"), ("content_type", "text/plain")])
def test_mutated_response_fields_are_readmitted_before_parsing(field, value):
    fixture = Fixture()
    def mutate(url, **kwargs):
        response = fixture(url, **kwargs)
        object.__setattr__(response, field, value)
        return response
    with pytest.raises((NciThesaurusRetrievalError, ReviewedPublicHttpError)):
        read(mutate)


@pytest.mark.parametrize("kwargs", [{"concept_code": "C3058"}, {"timeout_ms": True}, {"timeout_ms": 99}, {"timeout_ms": 30_001}])
def test_unreviewed_selector_or_invalid_timeout_refuses_before_dispatch(kwargs):
    with pytest.raises(NciThesaurusRetrievalError):
        NciThesaurusRetrievalConfig(**kwargs)


def test_mutated_frozen_config_is_revalidated_before_callback():
    config, fixture = NciThesaurusRetrievalConfig(), Fixture()
    object.__setattr__(config, "concept_code", "C3058")
    with pytest.raises(NciThesaurusRetrievalError):
        retrieve_nci_thesaurus_glioma(config, fetcher=fixture, approve_source_dispatch=True)
    assert not fixture.calls


def test_registration_is_inert_fixed_input_and_nonce_bound_for_application_callbacks():
    fixture = Fixture()
    first = register_nci_thesaurus_operations(WorkflowOperationCatalog(), fetcher=fixture)
    second = register_nci_thesaurus_operations(WorkflowOperationCatalog(), fetcher=Fixture())
    operation = first.resolve("nci_thesaurus.glioma_ontology")
    assert not fixture.calls and first.catalog_digest != second.catalog_digest
    assert operation.implementation == "caller_adapter" and operation.read_only is True
    assert set(operation.input_schema["properties"]) == {"timeout_ms"}
    assert operation.input_schema["additionalProperties"] is False


def test_registered_operation_enforces_approval_and_fixed_arguments_then_returns_replayable_native_bundle():
    fixture = Fixture()
    catalog = register_nci_thesaurus_operations(WorkflowOperationCatalog(), fetcher=fixture)
    for arguments, approval in [({}, False), ({"concept_code": "C3058"}, True), ({"limit": 1}, True)]:
        with pytest.raises(ValueError):
            catalog.execute("nci_thesaurus.glioma_ontology", arguments, approve_source_dispatch=approval, retrieved_at=CLOCK)
    assert not fixture.calls
    result = catalog.execute("nci_thesaurus.glioma_ontology", {"timeout_ms": 30_000}, approve_source_dispatch=True, retrieved_at=CLOCK)
    assert len(fixture.calls) == 1
    assert result.receipt["operation_id"] == "nci_thesaurus.glioma_ontology"
    assert result.receipt["bundle_digest"] == result.value["bundle_digest"]
    assert validate_nci_thesaurus_bundle(result.value) == result.value


def test_invalid_acquisition_clock_refuses_before_source_callback():
    fixture = Fixture()
    with pytest.raises(NciThesaurusRetrievalError, match="utc_timestamp"):
        retrieve_nci_thesaurus_glioma(fetcher=fixture, approve_source_dispatch=True, retrieved_at="2026-02-30T00:00:00Z")
    assert not fixture.calls


def test_default_acquisition_uses_shared_exact_route_and_replays_without_source_dispatch():
    fixture = Fixture()
    def get(client, route, *, approve_source_dispatch):
        assert route == "glioma_concept" and approve_source_dispatch is True
        return fixture(client.config.reviewed_routes[route], http_config=client.config)
    with patch("prism_sdk.reviewed_nci_thesaurus_retrieval.ReviewedPublicHttpClient.get", get):
        bundle = retrieve_nci_thesaurus_glioma(approve_source_dispatch=True, retrieved_at=CLOCK)
    assert len(fixture.calls) == 1 and bundle["transport"]["mode"] == "default_reviewed_public_https"
    assert validate_nci_thesaurus_bundle(bundle) == bundle
    assert len(fixture.calls) == 1
