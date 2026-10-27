"""Fixed annotation scope, missingness, native replay and transport identity invariants."""
from copy import deepcopy
import hashlib
import json
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.reviewed_public_http import ReviewedPublicHttpError
from prism_sdk.reviewed_reactome_retrieval import (
    ReactomeHttpResponse, ReactomeRetrievalConfig, ReactomeRetrievalError,
    register_reactome_operations, retrieve_reactome_pathways, validate_reactome_pathway_bundle,
)
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, builtin_workflow_operation_catalog

CLOCK = "2026-10-27T00:00:00Z"


def pathway(identifier=111448):
    return {"dbId": identifier, "displayName": "Fixture pathway annotation",
        "stId": f"R-HSA-{identifier}", "stIdVersion": f"R-HSA-{identifier}.5",
        "isInDisease": False, "isInferred": True, "maxDepth": 2,
        "name": ["Fixture pathway annotation"], "speciesName": "Homo sapiens",
        "schemaClass": "Pathway", "className": "Pathway", "hasDiagram": False,
        "hasEHLD": False, "releaseDate": "2004-10-27", "lastUpdatedDate": "2016-03-24"}


class Fixture:
    def __init__(self, value=None, *, body=None):
        self.value = [pathway()] if value is None else value
        self.body, self.calls = body, []

    def __call__(self, url, *, http_config):
        self.calls.append((url, http_config))
        body = json.dumps(self.value).encode() if self.body is None else self.body
        return ReactomeHttpResponse(200, body, url, len(body), hashlib.sha256(body).hexdigest(),
            http_config.transport_policy_digest, http_config.config_digest, "application/json")


def read(fixture=None):
    return retrieve_reactome_pathways(approve_source_dispatch=True, retrieved_at=CLOCK,
        fetcher=Fixture() if fixture is None else fixture)


def test_exact_fixed_human_route_retains_native_annotations_and_source_flags():
    fixture = Fixture()
    bundle = read(fixture)
    assert len(fixture.calls) == 1
    assert fixture.calls[0][0] == "https://reactome.org/ContentService/data/mapping/UniProt/P04637/pathways?species=9606"
    assert fixture.calls[0][1].max_response_bytes == 512_000
    assert bundle["pathways"][0]["native_record"] == pathway()
    assert bundle["pathways"][0]["source_is_inferred"] is True
    assert bundle["coverage"]["pathway_activity_measured"] is False
    assert bundle["coverage"]["reported_total_count"] is None
    assert validate_reactome_pathway_bundle(bundle) == bundle
    assert bundle["transport"]["mode"] == "caller_supplied"


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_source_approval_precedes_any_callback(approval):
    fixture = Fixture()
    with pytest.raises(ReactomeRetrievalError):
        retrieve_reactome_pathways(approve_source_dispatch=approval, fetcher=fixture)
    assert not fixture.calls


def test_successful_empty_mapping_is_observed_zero_with_unknown_completeness():
    bundle = read(Fixture([]))
    assert bundle["coverage"]["state"] == "empty_success"
    assert bundle["coverage"]["returned_pathway_count"] == 0
    assert bundle["coverage"]["exhaustiveness"] == "unassessed"
    assert validate_reactome_pathway_bundle(bundle) == bundle


@pytest.mark.parametrize("key,value", [
    ("speciesName", "Mus musculus"), ("schemaClass", "Reaction"), ("className", "Reaction"),
    ("dbId", True), ("dbId", 0), ("stId", "R-MMU-111448"), ("stIdVersion", "R-HSA-111448.0"),
    ("isInDisease", 0), ("isInferred", "false"), ("hasDiagram", None), ("name", []),
    ("name", [None]), ("displayName", ""), ("maxDepth", -1), ("releaseDate", "2026-02-30"),
    ("lastUpdatedDate", 1), ("unknownNativeField", "unsupported"),
])
def test_malformed_or_out_of_scope_native_rows_refuse_whole_response(key, value):
    row = pathway()
    row[key] = value
    with pytest.raises(ReactomeRetrievalError):
        read(Fixture([row]))


@pytest.mark.parametrize("body", [b'{"x":1}', b'null', b'[{"dbId":1,"dbId":2}]', b'[NaN]', b'\xff', b'', b'[] trailing'])
def test_non_array_duplicate_nonfinite_or_malformed_json_never_becomes_empty_success(body):
    with pytest.raises((ReactomeRetrievalError, ReviewedPublicHttpError)):
        read(Fixture(body=body))


def test_duplicate_native_identifiers_and_row_bound_refuse_without_truncation():
    for rows in ([pathway(), pathway()], [pathway(i+1) for i in range(257)]):
        with pytest.raises(ReactomeRetrievalError):
            read(Fixture(rows))


@pytest.mark.parametrize("field", ["pathways", "coverage", "receipts", "query", "limitations", "transport"])
def test_restamped_derived_tampering_still_fails_native_replay(field):
    value = read()
    if isinstance(value[field], list):
        value[field] = []
    else:
        value[field]["tampered"] = True
    value["bundle_digest"] = content_digest({key: child for key, child in value.items() if key != "bundle_digest"})
    with pytest.raises(ReactomeRetrievalError):
        validate_reactome_pathway_bundle(value)


def test_rate_limit_failure_is_retained_as_failure_with_one_call_and_no_retry():
    calls = []
    def fail(url, **_kwargs):
        calls.append(url)
        raise ReviewedPublicHttpError("http_status_not_success", status_code=429, retry_after_seconds=5)
    with pytest.raises(ReviewedPublicHttpError) as error:
        retrieve_reactome_pathways(fetcher=fail, approve_source_dispatch=True)
    assert len(calls) == 1
    assert error.value.status_code == 429 and error.value.automatic_retry is False


def test_application_callback_failure_is_sanitized_and_never_retried():
    calls = []
    def fail(url, **_kwargs):
        calls.append(url)
        raise RuntimeError("private callback failure text")
    with pytest.raises(ReactomeRetrievalError) as error:
        retrieve_reactome_pathways(fetcher=fail, approve_source_dispatch=True)
    assert str(error.value) == "reactome_source_request_failed" and len(calls) == 1
    assert error.value.__suppress_context__ is True


def test_elapsed_application_deadline_cannot_be_reported_as_success():
    with patch("prism_sdk.reviewed_reactome_retrieval.monotonic", side_effect=[10.0, 41.0]):
        with pytest.raises(ReactomeRetrievalError, match="elapsed_deadline"):
            read()


@pytest.mark.parametrize("field,value", [("body", b'[]'), ("response_bytes", 0), ("config_digest", "0"*64),
    ("transport_policy_digest", "0"*64), ("request_url", "https://reactome.org/unreviewed")])
def test_mutated_typed_response_is_readmitted_and_mismatched_identity_is_refused(field, value):
    fixture = Fixture()
    def mutate(url, **kwargs):
        response = fixture(url, **kwargs)
        object.__setattr__(response, field, value)
        return response
    with pytest.raises((ReactomeRetrievalError, ReviewedPublicHttpError)):
        read(mutate)


def test_injected_registration_has_nonce_identity_and_caller_adapter_label():
    first = register_reactome_operations(WorkflowOperationCatalog(), fetcher=Fixture())
    second = register_reactome_operations(WorkflowOperationCatalog(), fetcher=Fixture())
    assert first.catalog_digest != second.catalog_digest
    assert first.resolve("reactome.pathway_annotations").implementation == "caller_adapter"
    assert len(builtin_workflow_operation_catalog().operations()) == 8


@pytest.mark.parametrize("kwargs", [{"uniprot_accession": "P04637-1"}, {"timeout_ms": True}, {"timeout_ms": 30_001}])
def test_unreviewed_selectors_or_deadlines_refuse_before_dispatch(kwargs):
    with pytest.raises(ReactomeRetrievalError):
        ReactomeRetrievalConfig(**kwargs)


def test_builtin_acquisition_uses_shared_exact_route_and_deterministic_binding():
    fixture = Fixture()
    def get(client, route, *, approve_source_dispatch):
        assert route == "tp53_human_pathways" and approve_source_dispatch is True
        return fixture(client.config.reviewed_routes[route], http_config=client.config)
    with patch("prism_sdk.reviewed_reactome_retrieval.ReviewedPublicHttpClient.get", get):
        value = retrieve_reactome_pathways(approve_source_dispatch=True, retrieved_at=CLOCK)
    assert value["transport"]["mode"] == "default_reviewed_public_https"
    assert validate_reactome_pathway_bundle(value) == value
