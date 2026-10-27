"""Synthetic admission/refusal fixtures only; no observed live DataCite conformance."""
import base64
import copy
import hashlib
import json
from unittest.mock import Mock

import pytest

from prism_sdk import reviewed_datacite_collection_metadata_retrieval as dc
from prism_sdk import reviewed_public_http as wire
from prism_sdk.authoring import canonical_json, content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from test_reviewed_public_http import transport, wire as response_wire

NOW = "2026-10-27T22:00:00Z"
OPTIONAL = tuple(key for key in dc._FIELDS if key != "doi")


@pytest.fixture(autouse=True)
def deny_live_network_models_and_catalogue_execution(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("unapproved network/model/catalogue execution")
    monkeypatch.setattr(wire.socket, "socket", denied)
    monkeypatch.setattr(wire.socket, "getaddrinfo", denied)
    monkeypatch.setattr(WorkflowOperationCatalog, "execute", denied)
    for name in ("invoke", "invoke_stream", "_post_once"):
        monkeypatch.setattr(LLMRuntime, name, denied)


def native():
    return {"data": {"id": dc._DOI.lower(), "type": "dois", "attributes": {
        "doi": dc._DOI, "titles": [{"title": "Synthetic second title", "lang": "xx"},
            {"title": "Synthetic α title", "titleType": "Synthetic native label"}],
        "publisher": {"name": "Synthetic publisher", "schemeUri": "https://inert.invalid/scheme",
            "publisherIdentifier": "synthetic-native-id", "publisherIdentifierScheme": "Synthetic scheme",
            "uninterpreted": [1, 1.0, "1", False, None]},
        "publicationYear": 2016, "types": {"resourceTypeGeneral": "Dataset", "syntheticUnknown": [False, 0]},
        "version": "synthetic dataset v09", "metadataVersion": 42,
        "schemaVersion": "https://inert.invalid/schema/kernel-04", "state": "registered", "isActive": False,
        "created": "synthetic-created-source-string", "registered": "synthetic-registered-source-string",
        "updated": "synthetic-updated-source-string", "url": "https://inert.invalid/collection/TCGA-GBM",
        "contentUrl": ["https://inert.invalid/media"],
        "relatedIdentifiers": [{"relatedIdentifier": "synthetic-other-DOI", "relationType": "Synthetic relation"}],
        "subjects": [{"subject": "synthetic metadata keyword; not a patient record"}],
    }, "relationships": {"client": {"data": {"id": "synthetic-client", "type": "clients"}}}},
        "links": {"self": "https://inert.invalid/self"}, "meta": {"syntheticUnassessed": [True, 0]}}


def body(value):
    return json.dumps(value, ensure_ascii=False, indent=2).encode("utf-8")


def typed(raw, config, **changes):
    return dc.DataCiteCollectionMetadataHttpResponse(**({"status": 200, "body": raw, "request_url": dc._URL,
        "response_bytes": len(raw), "body_sha256": hashlib.sha256(raw).hexdigest(),
        "transport_policy_digest": wire.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
        "config_digest": config.config_digest, "content_type": "application/vnd.api+json"} | changes))


def retrieve(value=None, *, raw=None, changes=None):
    raw = body(native() if value is None else value) if raw is None else raw
    def fetch(url, *, http_config):
        assert url == dc._URL
        return typed(raw, http_config, **(changes or {}))
    return dc.retrieve_datacite_tcga_gbm_collection_metadata(approve_source_dispatch=True, retrieved_at=NOW, fetcher=fetch)


def attrs(value):
    return value["data"]["attributes"]


def resolve(value, pointer):
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def restamp(value):
    value["bundle_digest"] = content_digest({key: item for key, item in value.items() if key != "bundle_digest"})
    return value


@pytest.mark.parametrize("media", ["application/json", "application/vnd.api+json"])
def test_synthetic_default_wire_uses_one_exact_anonymous_get_and_retains_actual_json_media(media):
    raw = body(native())
    config = dc._http_config(dc.DataCiteCollectionMetadataRetrievalConfig())
    assert config.accepted_content_types == ("application/json", "application/vnd.api+json")
    assert config.allowed_hosts == ("api.datacite.org",)
    assert config.max_response_bytes == 524288 and config.timeout_ms == 30000
    with transport([response_wire(raw, content_type=media.encode())], configuration=config) as (_client, _raw, secured, dns, _context, _socket):
        value = dc.retrieve_datacite_tcga_gbm_collection_metadata(approve_source_dispatch=True, retrieved_at=NOW)
    sent = bytes(secured.sent)
    assert sent.startswith(b"GET /dois/10.7937/K9/TCIA.2016.RNYFUYE9 HTTP/1.1\r\nHost: api.datacite.org\r\n")
    assert sent.endswith(b"\r\n\r\n") and sent.count(b"GET ") == 1
    assert b"Authorization:" not in sent and b"Cookie:" not in sent and b"?" not in sent
    assert dns.call_count == 1 and value["receipts"][0]["source_request_count"] == 1
    assert value["transport"]["mode"] == "default_reviewed_public_http"
    assert value["transport"]["timing_authority"] == "builtin_total_deadline"
    assert value["source_inputs"][0]["typed_transport_metadata"]["content_type"] == media
    assert dc.validate_datacite_collection_metadata_bundle(value) == value


def test_raw_native_types_order_unknown_attributes_and_all_eighteen_citations_replay_without_execution(monkeypatch):
    source = native()
    raw = body(source) + b"\n \t"
    value = retrieve(raw=raw)
    retained, record = value["source_inputs"][0], value["records"][0]
    assert base64.b64decode(retained["response_body_base64"], validate=True) == raw
    assert retained["response_digest"] == hashlib.sha256(raw).hexdigest()
    assert canonical_json(retained["native_payload"]) == canonical_json(source)
    assert record["native_id"] == dc._DOI.lower() and record["native_doi"] == dc._DOI
    assert record["observations"]["titles"]["value"] == attrs(source)["titles"]
    assert record["unprojected_native_attribute_keys"] == ["contentUrl", "relatedIdentifiers", "subjects"]
    mixed = record["observations"]["publisher"]["value"]["uninterpreted"]
    assert [type(item) for item in mixed] == [int, float, str, bool, type(None)]
    assert len(value["citation_index"]) == 18
    for citation in value["citation_index"]:
        assert citation["artifact_pointer_root"] == "validated_datacite_bundle"
        observed = resolve(value, citation["artifact_pointer"])
        assert canonical_json(observed) == canonical_json(citation["source_value"]) == canonical_json(resolve(source, citation["native_payload_pointer"]))
        assert citation["source_value_digest"] == content_digest(observed)
        assert citation["citation_id"] == "datacite_native:" + content_digest({key: item for key, item in citation.items() if key != "citation_id"})
        assert citation["native_doi"] == dc._DOI and citation["retrieved_at"] == NOW
    monkeypatch.setattr(wire.ReviewedPublicHttpClient, "get", Mock(side_effect=AssertionError("replay dispatched")))
    assert dc.validate_datacite_collection_metadata_bundle(json.loads(canonical_json(value))) == value


@pytest.mark.parametrize("publisher", ["  Synthetic native publisher α  ", "", {}, {"unknown": {"ordered": [False, 0, 1.0]}}])
def test_synthetic_publisher_string_and_complete_opaque_object_variants_are_never_flattened(publisher):
    source = native()
    attrs(source)["publisher"] = publisher
    observation = retrieve(source)["records"][0]["observations"]["publisher"]
    assert observation["state"] == "observed"
    assert type(observation["value"]) is type(publisher)
    assert canonical_json(observation["value"]) == canonical_json(publisher)


@pytest.mark.parametrize("field", OPTIONAL)
def test_each_optional_missing_and_null_has_distinct_native_or_containing_absence_citation(field):
    absent, explicit = native(), native()
    del attrs(absent)[field]
    attrs(explicit)[field] = None
    a, b = retrieve(absent), retrieve(explicit)
    missing, null = a["records"][0]["observations"][field], b["records"][0]["observations"][field]
    assert missing["state"] == "missing" and missing["native_pointer"] is missing["value_citation_id"] is None
    context = next(row for row in a["citation_index"] if row["citation_id"] == missing["absence_context_citation_id"])
    assert context["native_payload_pointer"] == "/data/attributes" and field not in context["source_value"]
    assert null["state"] == "null" and null["native_pointer"].endswith("/" + field)
    assert null["value"] is None and null["value_citation_id"] and null["absence_context_citation_id"] is None


@pytest.mark.parametrize("field,empty", [("titles", []), ("publisher", {}), ("publisher", ""), ("types", {}),
    *((name, "") for name in dc._TEXT_FIELDS), ("publicationYear", 0), ("metadataVersion", 0), ("isActive", False)])
def test_synthetic_empty_zero_and_false_are_observed_values_without_missingness_or_quality_promotion(field, empty):
    source = native()
    attrs(source)[field] = empty
    value = retrieve(source)
    observed = value["records"][0]["observations"][field]
    assert observed["state"] == "observed" and type(observed["value"]) is type(empty)
    assert canonical_json(observed["value"]) == canonical_json(empty)
    assert value["coverage"]["returned_record_count"] == len(value["records"]) == 1
    assert value["scientific_support_established"] is value["records"][0]["scientific_quality_assessed"] is False


@pytest.mark.parametrize("state", ["findable", "registered", "draft", "unfamiliar native state", ""])
def test_inactive_and_nonfindable_native_states_remain_explicit_not_empty_success_or_scientific_absence(state):
    source = native()
    attrs(source)["state"], attrs(source)["isActive"] = state, False
    value = retrieve(source)
    assert len(value["records"]) == 1
    assert value["records"][0]["observations"]["state"]["value"] == state
    assert value["records"][0]["observations"]["isActive"]["value"] is False
    coverage = value["coverage"]
    assert coverage["source_metadata_record_completeness"] == coverage["completeness"] == coverage["omitted_influence"] == "unknown"
    assert coverage["native_response_schema_conformance"] == "unassessed" and coverage["linked_resource_requests"] == 0
    assert all(coverage[key] is None for key in ("dataset_release_id", "patient_or_subject_count", "image_or_series_count", "cohort_or_independent_study_count", "independent_publication_count"))
    assert value["clinical_actions_authorized"] is value["laboratory_actions_authorized"] is value["imaging_access_authorized"] is False


def test_dataset_version_record_revision_and_schema_identifier_cannot_be_conflated_or_promoted_to_current_release():
    source = native()
    value = retrieve(source)
    record = value["records"][0]
    assert record["dataset_version_observation"] == record["observations"]["version"]
    assert record["metadata_revision_observation"] == record["observations"]["metadataVersion"]
    assert record["metadata_schema_observation"] == record["observations"]["schemaVersion"]
    assert [record[key]["value"] for key in ("dataset_version_observation", "metadata_revision_observation", "metadata_schema_observation")] == ["synthetic dataset v09", 42, "https://inert.invalid/schema/kernel-04"]
    assert value["coverage"]["dataset_release_id"] is None and value["coverage"]["source_snapshot_identity"] == "unestablished"


@pytest.mark.parametrize("identifier", [dc._DOI, dc._DOI.lower(), "10.7937/k9/TCIA.2016.rnyfuye9"])
def test_documented_ascii_case_equivalence_admits_both_identity_fields_without_rewriting(identifier):
    source = native()
    source["data"]["id"] = attrs(source)["doi"] = identifier
    record = retrieve(source)["records"][0]
    assert record["native_id"] == record["native_doi"] == identifier


@pytest.mark.parametrize("field", ["id", "doi"])
@pytest.mark.parametrize("identifier", [" " + dc._DOI, dc._DOI + " ", "https://doi.org/" + dc._DOI,
    "10.7937/K9/TCIA.2016.RNYFUYE9", "10.7937/K9/TCIA.2016.RNYFUYE0", "", None, 7])
def test_no_trim_url_unwrap_unicode_casefold_or_wrong_identity_is_admitted(field, identifier):
    source = native()
    (source["data"] if field == "id" else attrs(source))[field] = identifier
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("change", ["array_data", "missing_id", "missing_doi", "missing_type", "wrong_type", "missing_attributes",
    "errors", "included", "unknown_root", "unknown_resource", "bad_links", "bad_relationships"])
def test_non_singleton_error_or_unreviewed_wrapper_shapes_refuse_whole_artifact(change):
    source = native()
    if change == "array_data": source["data"] = [source["data"]]
    elif change == "missing_id": del source["data"]["id"]
    elif change == "missing_doi": del attrs(source)["doi"]
    elif change == "missing_type": del source["data"]["type"]
    elif change == "wrong_type": source["data"]["type"] = "collections"
    elif change == "missing_attributes": del source["data"]["attributes"]
    elif change == "errors": source["errors"] = []
    elif change == "included": source["included"] = []
    elif change == "unknown_root": source["unreviewed"] = {}
    elif change == "unknown_resource": source["data"]["unreviewed"] = {}
    elif change == "bad_links": source["links"] = []
    elif change == "bad_relationships": source["data"]["relationships"] = "opaque link"
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("field,bad", [("publisher", []), ("publisher", 0), ("types", []), ("titles", {}),
    ("titles", [{"lang": "xx"}]), ("titles", [{"title": ""}]), ("titles", [{"title": "x"}] * 65),
    ("publicationYear", True), ("publicationYear", 2016.0), ("publicationYear", "2016"), ("publicationYear", -1), ("publicationYear", 10000),
    ("metadataVersion", True), ("metadataVersion", 1.0), ("metadataVersion", -1), ("metadataVersion", 9223372036854775808),
    ("isActive", 0), ("isActive", "false"), ("version", 9), ("version", "x" * 4097),
    ("schemaVersion", {}), ("state", "x" * 129), ("publisher", "x" * 32769), ("url", "a\x00b")],
    ids=["publisher-list", "publisher-integer", "types-list", "titles-object", "title-missing", "title-empty", "titles-over-count",
        "year-boolean", "year-float", "year-string", "year-negative", "year-over-max", "revision-boolean", "revision-float",
        "revision-negative", "revision-over-max", "active-integer", "active-string", "version-number", "version-over-bytes",
        "schema-object", "state-over-bytes", "publisher-over-bytes", "url-nul"])
def test_selected_type_array_integer_and_string_limits_are_local_whole_refusal_policy(field, bad):
    source = native()
    attrs(source)[field] = bad
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("raw", [b'{"data":{},"data":{}}', b'{"opaque":NaN}', b'{"opaque":1e999}', b'\xff', b'[]', b''])
def test_duplicate_nonfinite_invalid_utf8_or_nonsingleton_json_never_becomes_empty_success(raw):
    with pytest.raises((dc.DataCiteCollectionMetadataRetrievalError, wire.ReviewedPublicHttpError)):
        retrieve(raw=raw)


@pytest.mark.parametrize("change", ["depth", "nodes", "key", "response_bytes"])
def test_complete_raw_tree_limits_refuse_without_truncating_unprojected_metadata(change):
    source = native()
    if change == "depth":
        nested = 0
        for _ in range(22): nested = [nested]
        attrs(source)["unprojected"] = nested
    elif change == "nodes": attrs(source)["unprojected"] = [0] * 30000
    elif change == "key": attrs(source)["x" * 257] = "native value"
    else: attrs(source)["unprojected"] = "x" * dc.MAX_DATACITE_RESPONSE_BYTES
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        retrieve(source)


def test_maximum_raw_byte_fixture_retains_all_unknown_padding_and_eighteen_citations_without_live_claim():
    source = native()
    attrs(source)["syntheticPadding"] = ""
    attrs(source)["syntheticPadding"] = "x" * (dc.MAX_DATACITE_RESPONSE_BYTES - len(body(source)))
    raw = body(source)
    assert len(raw) == 524288
    value = retrieve(raw=raw)
    assert base64.b64decode(value["source_inputs"][0]["response_body_base64"]) == raw
    assert len(value["citation_index"]) == 18
    assert len(canonical_json(value).encode()) <= dc.MAX_DATACITE_BUNDLE_BYTES
    assert dc.validate_datacite_collection_metadata_bundle(value) == value


@pytest.mark.parametrize("policy", ["citations", "artifact"])
def test_synthetic_lowered_policy_caps_reach_whole_artifact_refusal_instead_of_prefix_output(monkeypatch, policy):
    monkeypatch.setattr(dc, "MAX_DATACITE_CITATIONS" if policy == "citations" else "MAX_DATACITE_BUNDLE_BYTES", 17 if policy == "citations" else 2000)
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError, match="citation_bound|artifact_byte_bound"):
        retrieve()


@pytest.mark.parametrize("change", ["publisher", "native_payload", "dataset_version", "record_revision", "schema_identifier", "missingness",
    "citation", "total", "omission", "authorization", "default_mode", "base64", "content_type", "clock", "query"])
def test_restamped_projection_citation_scope_or_provenance_changes_do_not_survive_raw_reconstruction(change):
    value = copy.deepcopy(retrieve())
    record = value["records"][0]
    if change == "publisher": record["observations"]["publisher"]["value"] = "changed"
    elif change == "native_payload": attrs(value["source_inputs"][0]["native_payload"])["version"] = "changed"
    elif change == "dataset_version": record["dataset_version_observation"]["value"] = "changed"
    elif change == "record_revision": record["metadata_revision_observation"]["value"] = 43
    elif change == "schema_identifier": record["metadata_schema_observation"]["value"] = "changed"
    elif change == "missingness": record["observations"]["publisher"]["state"] = "missing"
    elif change == "citation": value["citation_index"][0]["source_value"] = {}
    elif change == "total": value["coverage"]["cohort_or_independent_study_count"] = 1
    elif change == "omission": value["omissions"][0]["influence"] = "zero"
    elif change == "authorization": value["imaging_access_authorized"] = True
    elif change == "default_mode": value["transport"]["mode"] = "default_reviewed_public_http"
    elif change == "base64": value["source_inputs"][0]["response_body_base64"] += "="
    elif change == "content_type": value["source_inputs"][0]["typed_transport_metadata"]["content_type"] = "application/problem+json"
    elif change == "clock": value["retrieved_at"] = "2026-10-28T00:00:00Z"
    elif change == "query": value["query"]["doi"] = "10.0/other"
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        dc.validate_datacite_collection_metadata_bundle(restamp(value))


def test_coherent_whole_source_replacement_stays_caller_assertion_and_is_not_authenticated_truth():
    value = retrieve()
    replacement = native()
    attrs(replacement)["publisher"] = "Coherent synthetic replacement"
    rebuilt = dc._bundle(dc.DataCiteCollectionMetadataRetrievalConfig(), body=body(replacement), retrieved_at=NOW,
        content_type="application/json", mode="caller_supplied", binding=value["transport"]["binding_id"])
    assert dc.validate_datacite_collection_metadata_bundle(rebuilt) == rebuilt
    assert rebuilt["source_origin_authentication"] == "unestablished" and rebuilt["transport"]["mode"] == "caller_supplied"


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_approval_is_required_before_any_default_or_caller_transport(approval):
    fetcher = Mock(side_effect=AssertionError("approval bypass"))
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError, match="approval"):
        dc.retrieve_datacite_tcga_gbm_collection_metadata(approve_source_dispatch=approval, fetcher=fetcher)
    fetcher.assert_not_called()


@pytest.mark.parametrize("timeout", [True, None, 99, 30001, 100.0, "30000"])
def test_timeout_is_bounded_exact_integer_without_coercion(timeout):
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        dc.DataCiteCollectionMetadataRetrievalConfig(timeout)


@pytest.mark.parametrize("changes", [{"config_digest": "0" * 64}, {"transport_policy_digest": "0" * 64},
    {"request_url": dc._URL + "?enriched=true"}, {"request_url": "https://other.invalid/metadata"}, {"content_type": "application/problem+json"}])
def test_typed_transport_binding_media_and_exact_scope_changes_refuse(changes):
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError):
        retrieve(changes=changes)


def test_one_failed_or_late_caller_request_never_retries_falls_back_or_leaks_private_details(monkeypatch):
    fetcher = Mock(side_effect=RuntimeError("private synthetic detail"))
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError, match="source_request_failed") as caught:
        dc.retrieve_datacite_tcga_gbm_collection_metadata(approve_source_dispatch=True, fetcher=fetcher)
    assert "private synthetic detail" not in str(caught.value) and fetcher.call_count == 1
    ticks = iter([0.0, 31.0])
    monkeypatch.setattr(dc, "monotonic", lambda: next(ticks))
    with pytest.raises(dc.DataCiteCollectionMetadataRetrievalError, match="deadline"):
        retrieve()


def test_default_wire_redirect_or_rate_limit_refuses_after_one_request_without_traversal():
    config = dc._http_config(dc.DataCiteCollectionMetadataRetrievalConfig())
    for status in (302, 429):
        with transport([response_wire(b'{}', b"Location: https://inert.invalid/next\r\nRetry-After: 1\r\n", status=status)], configuration=config) as (_client, _raw, secured, dns, *_rest):
            with pytest.raises(wire.ReviewedPublicHttpError):
                dc.retrieve_datacite_tcga_gbm_collection_metadata(approve_source_dispatch=True)
        assert bytes(secured.sent).count(b"GET ") == 1 and dns.call_count == 1


def test_registration_retains_caller_nonce_and_refuses_endpoint_query_or_authority_arguments_before_dispatch():
    first, second = WorkflowOperationCatalog(), WorkflowOperationCatalog()
    fetcher = Mock(side_effect=AssertionError("registration or validation dispatched"))
    dc.register_datacite_collection_metadata_operations(first, fetcher=fetcher)
    dc.register_datacite_collection_metadata_operations(second, fetcher=fetcher)
    assert first.catalog_digest != second.catalog_digest
    descriptor = first.resolve(dc.DATACITE_COLLECTION_METADATA_OPERATION_ID)
    assert descriptor.integration_id == "datacite" and descriptor.implementation == "caller_adapter" and descriptor.read_only is True
    assert set(descriptor.input_schema["properties"]) == {"timeout_ms"} and descriptor.input_schema["additionalProperties"] is False
    assert first.validate_arguments(descriptor.operation_id, {}) == {"timeout_ms": 30000}
    for key in ("url", "doi", "query", "enriched", "approve_source_dispatch"):
        with pytest.raises(ArgumentError):
            first.validate_arguments(descriptor.operation_id, {key: "unreviewed"})
    before = first.to_dict()
    with pytest.raises(ArgumentError): dc.register_datacite_collection_metadata_operations(first)
    assert first.to_dict() == before
    fetcher.assert_not_called()


def test_core_does_not_onboard_datacite_or_change_existing_manifest_default_catalogue_or_get_policy():
    from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
    from prism_sdk.workflow_provider_registry import reviewed_workflow_provider_registry
    base = builtin_workflow_operation_catalog()
    assert base.catalog_digest == "53e11ef585fa6d65736ecc77d5042962a9276fac8fbe7b39bde1efccfec5e5b2"
    registry = reviewed_workflow_provider_registry(base).to_dict()
    assert registry["counts"]["implemented_reader_paths"] == 20 and registry["counts"]["declared_reviewed_service_groups"] == 17
    assert dc.DATACITE_COLLECTION_METADATA_OPERATION_ID not in {row.operation_id for row in base.operations()}
    assert wire.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST == "07c73dc8fcba4fcdfa2cb08a941062f6a736cbcff38c02cdf457a09593a3f46e"
