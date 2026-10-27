"""Synthetic native contract fixtures; these tests establish no live/scientific coverage."""
import base64
import hashlib
import json
from unittest.mock import Mock

import pytest

from prism_sdk import reviewed_civic_evidence_retrieval as civic
from prism_sdk import reviewed_public_graphql as graphql
from prism_sdk.authoring import content_digest
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from test_reviewed_public_graphql import transport
from test_reviewed_public_http import wire as response_wire

NOW = "2026-10-27T18:00:00Z"


@pytest.fixture(autouse=True)
def deny_accidental_network(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("unapproved test network call")
    monkeypatch.setattr(graphql.wire.socket, "getaddrinfo", denied)
    monkeypatch.setattr(graphql.wire.socket, "socket", denied)
    monkeypatch.setattr(civic, "_NEXT_ADMISSION", 0.0)


def native(total=1):
    return {"data": {"evidenceItems": {
        "totalCount": total, "pageInfo": {"hasNextPage": total > 25, "endCursor": "inert-next" if total else None},
        "nodes": [{"id": 100 + i, "status": "ACCEPTED", "flagged": False, "openRevisionCount": 0,
            "evidenceType": "DIAGNOSTIC", "evidenceLevel": "B", "evidenceDirection": "SUPPORTS",
            "evidenceRating": 3, "disease": {"id": 23, "doid": "3068", "name": "glioblastoma"},
            "source": {"id": 50, "citationId": "12345678", "displayType": "PubMed"}}
            for i in range(min(total, 25))]}}}


def body(value=None):
    return json.dumps(native() if value is None else value, ensure_ascii=False, indent=2).encode("utf-8")


def typed(raw, http_config, **changes):
    return civic.CivicEvidenceHttpResponse(**({
        "status": 200, "body": raw, "request_url": civic._URL, "response_bytes": len(raw),
        "body_sha256": hashlib.sha256(raw).hexdigest(),
        "transport_policy_digest": graphql.REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST,
        "config_digest": http_config.config_digest, "content_type": "application/json"} | changes))


def retrieve(value=None, *, raw=None, changes=None):
    raw = body(value) if raw is None else raw
    def fetch(url, *, http_config):
        assert url == civic._URL
        return typed(raw, http_config, **(changes or {}))
    return civic.retrieve_civic_glioblastoma_evidence(approve_source_dispatch=True, retrieved_at=NOW, fetcher=fetch)


def pointer(document, path):
    value = document
    for token in path.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def restamp(bundle):
    bundle["bundle_digest"] = content_digest({key: value for key, value in bundle.items() if key != "bundle_digest"})
    return bundle


def test_default_reader_uses_actual_fixed_post_wire_parser_once_and_descriptor_is_inert():
    raw = body()
    cfg = civic._http_config(civic.CivicEvidenceRetrievalConfig())
    with transport([response_wire(raw, b"Content-Length: " + str(len(raw)).encode() + b"\r\n")],
                   config=cfg, send_limit=11) as (_client, _raw, secured, dns, context, _socket):
        bundle = civic.retrieve_civic_glioblastoma_evidence(approve_source_dispatch=True, retrieved_at=NOW)
    assert bundle["transport"]["mode"] == "default_reviewed_public_graphql"
    assert bundle["transport"]["timing_authority"] == "builtin_total_deadline"
    assert dns.call_count == 1
    context.return_value.wrap_socket.assert_called_once()
    header, sent_body = bytes(secured.sent).split(b"\r\n\r\n", 1)
    assert header.startswith(b"POST /api/graphql HTTP/1.1\r\nHost: civicdb.org\r\n")
    assert sent_body == civic._REQUEST_BODY
    assert hashlib.sha256(sent_body).hexdigest() == civic.CIVIC_EVIDENCE_REQUEST_BODY_SHA256
    request = json.loads(sent_body)
    assert request == {"query": civic.CIVIC_EVIDENCE_QUERY,
                       "operationName": civic.CIVIC_EVIDENCE_OPERATION_NAME, "variables": {}}
    assert "first: 25" in request["query"] and "status: ACCEPTED" in request["query"]
    for excluded in ("description", "therapies", "molecularProfile", "variantHgvs", "sequence", "patient"):
        assert excluded not in request["query"]
    assert bundle["receipts"][0]["source_request_count"] == 1
    assert bundle["scientific_support_established"] is False and bundle["clinical_actions_authorized"] is False


def test_exact_raw_bytes_all_native_pointers_and_request_digest_replay_without_network(monkeypatch):
    value = native(2)
    value["data"]["evidenceItems"]["nodes"][0]["disease"]["name"] = "glioblastoma α"
    raw = body(value) + b"\n "
    bundle = retrieve(raw=raw)
    source = bundle["source_inputs"][0]
    assert base64.b64decode(source["response_body_base64"], validate=True) == raw
    assert source["response_digest"] == hashlib.sha256(raw).hexdigest()
    assert source["native_payload"] == value
    assert source["request_body_utf8"].encode() == civic._REQUEST_BODY
    for record in bundle["records"]:
        assert pointer(value, record["native_pointer"])["id"] == record["evidence_id"]
        for observation in record["observations"].values():
            assert pointer(value, observation["native_pointer"]) == observation["value"]
        for observation in record["disease_fields"].values():
            assert pointer(value, observation["native_pointer"]) == observation["value"]
        for observation in record["source_fields"].values():
            assert pointer(value, observation["native_pointer"]) == observation["value"]
        assert pointer(value, record["citation"]["native_source_pointer"])["id"] == record["citation"]["source_id"]
    monkeypatch.setattr(graphql.ReviewedPublicGraphqlClient, "post", Mock(side_effect=AssertionError("replay dispatched")))
    assert civic.validate_civic_evidence_bundle(bundle) == bundle


@pytest.mark.parametrize("field", ["disease", "evidenceRating"])
def test_missing_nullable_item_field_stays_distinct_from_null_and_observed_zero(field):
    missing, null = native(), native()
    del missing["data"]["evidenceItems"]["nodes"][0][field]
    null["data"]["evidenceItems"]["nodes"][0][field] = None
    absent = retrieve(missing)
    explicit = retrieve(null)
    assert absent["records"][0]["observations"][field] == {"state": "missing", "value": None, "native_pointer": None}
    assert explicit["records"][0]["observations"][field]["state"] == "null"
    assert explicit["records"][0]["observations"][field]["native_pointer"].endswith("/" + field)
    assert absent["coverage"]["missing_nullable_item_fields"] == 1
    assert explicit["coverage"]["missing_nullable_item_fields"] == 0
    assert civic.validate_civic_evidence_bundle(absent) == absent
    assert civic.validate_civic_evidence_bundle(explicit) == explicit


def test_nullable_doid_and_cursor_missingness_is_retained_without_inherited_values():
    value = native()
    del value["data"]["evidenceItems"]["nodes"][0]["disease"]["doid"]
    del value["data"]["evidenceItems"]["pageInfo"]["endCursor"]
    bundle = retrieve(value)
    assert bundle["records"][0]["disease_fields"]["doid"]["state"] == "missing"
    assert bundle["coverage"]["end_cursor"] == {"state": "missing", "value": None, "native_pointer": None}
    assert bundle["coverage"]["completeness"] == "unknown"


@pytest.mark.parametrize("total", [0, 1, 25, 27])
def test_one_page_counts_never_become_independent_publication_or_patient_denominators(total):
    bundle = retrieve(native(total))
    coverage = bundle["coverage"]
    assert len(bundle["records"]) == min(total, 25)
    assert coverage["reported_filtered_total_count"] == total
    assert coverage["unreturned_reported_evidence_item_count"] == max(0, total - 25)
    assert coverage["has_next_page"] is (total > 25) and coverage["pagination_followed"] is False
    assert coverage["completeness"] == coverage["omitted_influence"] == "unknown"
    assert coverage["independent_publication_count"] is None
    assert coverage["patient_or_cohort_or_assay_denominator"] is None
    assert all(item["omitted_count"] is None and item["influence"] == "unknown" for item in bundle["omissions"])
    assert len({row["citation"]["source_id"] for row in bundle["records"]}) == (1 if total else 0)


@pytest.mark.parametrize("approval", [False, 1, "true", None])
def test_literal_source_approval_is_required_before_even_an_injected_fetcher(approval):
    fetch = Mock(side_effect=AssertionError("approval bypass"))
    with pytest.raises(civic.CivicEvidenceRetrievalError, match="approval"):
        civic.retrieve_civic_glioblastoma_evidence(approve_source_dispatch=approval, fetcher=fetch)
    fetch.assert_not_called()


@pytest.mark.parametrize("timeout", [True, 99, 30001, 100.0, "30000", None])
def test_timeout_config_does_not_coerce_missing_boolean_or_float_values(timeout):
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        civic.CivicEvidenceRetrievalConfig(timeout)


@pytest.mark.parametrize("field,value", [
    ("status", "SUBMITTED"), ("status", None), ("evidenceType", "NEW_TYPE"),
    ("evidenceLevel", "F"), ("evidenceDirection", "UNKNOWN"),
    ("flagged", 0), ("openRevisionCount", True), ("openRevisionCount", -1),
    ("id", 0), ("id", True), ("evidenceRating", 0), ("evidenceRating", 6),
    ("evidenceRating", True), ("source", None), ("disease", []),
])
def test_bad_native_row_is_refused_instead_of_discarded_or_defaulted(field, value):
    payload = native()
    payload["data"]["evidenceItems"]["nodes"][0][field] = value
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        retrieve(payload)


@pytest.mark.parametrize("field", sorted(civic._REQUIRED))
def test_missing_nonnullable_selected_field_is_not_a_successful_empty_value(field):
    payload = native()
    del payload["data"]["evidenceItems"]["nodes"][0][field]
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        retrieve(payload)


@pytest.mark.parametrize("change", ["partial_errors", "null_errors", "empty_errors", "null_data", "null_connection",
    "null_nodes", "null_item", "extra_variant", "duplicate_id", "descending_id", "short_page", "too_many",
    "bad_total", "bad_has_next", "contradict_next", "empty_cursor", "cursor_on_empty"])
def test_graphql_partial_errors_or_changed_shapes_cannot_masquerade_as_admitted_metadata(change):
    payload = native(2)
    connection = payload["data"]["evidenceItems"]
    if change in {"partial_errors", "null_errors", "empty_errors"}:
        payload["errors"] = {"partial_errors": [{"message": "partial fixture"}], "null_errors": None, "empty_errors": []}[change]
    elif change == "null_data": payload["data"] = None
    elif change == "null_connection": payload["data"]["evidenceItems"] = None
    elif change == "null_nodes": connection["nodes"] = None
    elif change == "null_item": connection["nodes"][0] = None
    elif change == "extra_variant": connection["nodes"][0]["variantHgvs"] = "excluded"
    elif change == "duplicate_id": connection["nodes"][1]["id"] = connection["nodes"][0]["id"]
    elif change == "descending_id": connection["nodes"].reverse()
    elif change == "short_page": connection["nodes"].pop()
    elif change == "too_many":
        connection["nodes"] *= 13
        connection["totalCount"] = 26
        connection["pageInfo"]["hasNextPage"] = True
    elif change == "bad_total": connection["totalCount"] = True
    elif change == "bad_has_next": connection["pageInfo"]["hasNextPage"] = 0
    elif change == "contradict_next": connection["pageInfo"]["hasNextPage"] = True
    elif change == "empty_cursor": connection["pageInfo"]["endCursor"] = ""
    elif change == "cursor_on_empty":
        payload = native(0)
        payload["data"]["evidenceItems"]["pageInfo"]["endCursor"] = "unexpected"
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        retrieve(payload)


@pytest.mark.parametrize("raw", [b'{"data":{},"data":{}}', b'{"data":NaN}', b"\xff", b"{", b"[]", b"null", b""])
def test_malformed_duplicate_or_nonfinite_json_is_not_an_empty_page(raw):
    expected = graphql.ReviewedPublicGraphqlError if raw == b"\xff" else civic.CivicEvidenceRetrievalError
    with pytest.raises(expected):
        retrieve(raw=raw)


@pytest.mark.parametrize("change", ["projection", "payload", "raw_bytes", "query", "request_digest", "pagination",
    "timestamp", "default_mode", "base64"])
def test_self_restamping_cannot_conceal_disagreement_with_retained_bytes_and_fixed_request(change):
    bundle = retrieve()
    if change == "projection": bundle["records"][0]["evidence_id"] = 999
    elif change == "payload": bundle["source_inputs"][0]["native_payload"]["data"]["evidenceItems"]["totalCount"] = 999
    elif change == "raw_bytes": bundle["source_inputs"][0]["response_body_base64"] = base64.b64encode(body(native(0))).decode()
    elif change == "query": bundle["query"]["query"] += " mutation NotAllowed { x }"
    elif change == "request_digest": bundle["source_inputs"][0]["request_body_sha256"] = "0" * 64
    elif change == "pagination": bundle["coverage"]["has_next_page"] = True
    elif change == "timestamp": bundle["retrieved_at"] = "yesterday"
    elif change == "default_mode": bundle["transport"]["mode"] = "default_reviewed_public_graphql"
    elif change == "base64": bundle["source_inputs"][0]["response_body_base64"] += "="
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        civic.validate_civic_evidence_bundle(restamp(bundle))


@pytest.mark.parametrize("changes", [
    {"config_digest": "0" * 64}, {"transport_policy_digest": "0" * 64},
    {"request_url": "https://civicdb.org/api/other"}, {"content_type": "application/problem+json"},
])
def test_typed_response_with_wrong_fixed_scope_or_policy_is_refused(changes):
    with pytest.raises(civic.CivicEvidenceRetrievalError):
        retrieve(changes=changes)


def test_caller_errors_are_sanitized_and_late_return_cannot_become_an_admitted_bundle(monkeypatch):
    fetch = Mock(side_effect=RuntimeError("private fixture detail"))
    with pytest.raises(civic.CivicEvidenceRetrievalError) as failure:
        civic.retrieve_civic_glioblastoma_evidence(approve_source_dispatch=True, fetcher=fetch)
    assert "private fixture" not in str(failure.value)
    monkeypatch.setattr(civic, "monotonic", Mock(side_effect=[0.0, .101]))
    with pytest.raises(civic.CivicEvidenceRetrievalError, match="elapsed_deadline"):
        civic.retrieve_civic_glioblastoma_evidence(civic.CivicEvidenceRetrievalConfig(100),
            approve_source_dispatch=True, retrieved_at=NOW,
            fetcher=lambda _url, *, http_config: typed(body(), http_config))


def test_default_local_rate_admission_refuses_early_start_without_wait_or_retry(monkeypatch):
    monkeypatch.setattr(civic, "monotonic", Mock(side_effect=[10.0, 10.5, 11.0]))
    civic._admit_start()
    with pytest.raises(civic.CivicEvidenceRetrievalError, match="rate_admission"):
        civic._admit_start()
    civic._admit_start()


def test_registration_is_inert_default_binding_stable_and_caller_binding_is_not_reviewed_default():
    first = civic.register_civic_evidence_operations(WorkflowOperationCatalog())
    second = civic.register_civic_evidence_operations(WorkflowOperationCatalog())
    assert first.to_dict() == second.to_dict()
    assert first.resolve(civic.CIVIC_EVIDENCE_OPERATION_ID).implementation == "live_adapter"
    fake = Mock()
    caller = civic.register_civic_evidence_operations(WorkflowOperationCatalog(), fetcher=fake)
    assert caller.resolve(civic.CIVIC_EVIDENCE_OPERATION_ID).implementation == "caller_adapter"
    assert caller.to_dict() != first.to_dict()
    fake.assert_not_called()


@pytest.mark.parametrize("arguments", [{"query": "query {x}"}, {"cursor": "next"}, {"url": civic._URL},
    {"credentials": "not-requested"}, {"timeout_ms": True}, {"timeout_ms": 30000.0}])
def test_catalogue_rejects_query_endpoint_cursor_and_credential_expansion_before_fetch(arguments):
    fetch = Mock(side_effect=AssertionError("invalid argument dispatched"))
    catalog = civic.register_civic_evidence_operations(WorkflowOperationCatalog(), fetcher=fetch)
    with pytest.raises(ValueError):
        catalog.execute(civic.CIVIC_EVIDENCE_OPERATION_ID, arguments, approve_source_dispatch=True)
    fetch.assert_not_called()


def test_native_catalogue_receipt_binds_one_call_and_bundle_without_a_denominator_claim():
    fetch = Mock(side_effect=lambda _url, *, http_config: typed(body(), http_config))
    catalog = civic.register_civic_evidence_operations(WorkflowOperationCatalog(), fetcher=fetch)
    result = catalog.execute(civic.CIVIC_EVIDENCE_OPERATION_ID, {}, approve_source_dispatch=True, retrieved_at=NOW)
    assert fetch.call_count == 1
    assert result.receipt["operation_id"] == civic.CIVIC_EVIDENCE_OPERATION_ID
    assert result.receipt["bundle_digest"] == result.value["bundle_digest"]
    assert result.value["coverage"]["patient_or_cohort_or_assay_denominator"] is None


def test_reader_response_byte_bound_applies_before_json_admission():
    with pytest.raises(civic.CivicEvidenceRetrievalError, match="response_byte_bound"):
        retrieve(raw=b" " * (civic.MAX_CIVIC_RESPONSE_BYTES + 1))

