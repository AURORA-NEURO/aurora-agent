"""Synthetic contract boundaries only; no live adapter or biological validity is measured."""
import base64
import copy
import hashlib
import json
from unittest.mock import Mock

import pytest

from prism_sdk import reviewed_cellosaurus_cell_line_retrieval as cell
from prism_sdk import reviewed_public_http as wire
from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from test_reviewed_public_http import transport, wire as response_wire

NOW = "2026-10-27T20:00:00Z"


@pytest.fixture(autouse=True)
def deny_live_network_models_and_catalogue_dispatch(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("unapproved network/model/catalogue call")
    monkeypatch.setattr(wire.socket, "socket", denied)
    monkeypatch.setattr(wire.socket, "getaddrinfo", denied)
    monkeypatch.setattr(WorkflowOperationCatalog, "execute", denied)
    for name in ("invoke", "invoke_stream", "_post_once"):
        monkeypatch.setattr(LLMRuntime, name, denied)


def reference(database, accession, label):
    return {"database": database, "accession": accession, "label": label,
        "category": "Synthetic source label", "iri": "http://inert.invalid/native",
        "url": "https://inert.invalid/selected-reference"}


def native(count=1):
    return {"Cellosaurus": {"cell-line-list": [{
        "accession-list": [{"type": "primary", "value": f"SYNTHETIC_{i:04}"}],
        "name-list": [{"type": "synonym", "value": "synthetic alias"},
                      {"type": "identifier", "value": f"synthetic identity {i}"}],
        "species-list": [reference("NCBI_TaxID", "9606", "Synthetic human assertion")],
        "disease-list": [reference("NCIt", "SYNTHETIC_CONCEPT", "Synthetic disease assertion")],
        "comment-list": [{"category": "Problematic cell line", "value": "Synthetic source caution; no quality interpretation."}],
        "created": "synthetic-date-string", "last-updated": "unchanged source label", "entry-version": "00050",
    } for i in range(count)]}}


def body(value):
    return json.dumps(value, ensure_ascii=False, indent=2).encode("utf-8")


def typed(raw, config, **changes):
    return cell.CellosaurusCellLineHttpResponse(**({"status": 200, "body": raw, "request_url": cell._URL,
        "response_bytes": len(raw), "body_sha256": hashlib.sha256(raw).hexdigest(),
        "transport_policy_digest": wire.REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
        "config_digest": config.config_digest, "content_type": "application/json"} | changes))


def retrieve(value=None, *, raw=None, changes=None):
    raw = body(native() if value is None else value) if raw is None else raw
    def fetch(url, *, http_config):
        assert url == cell._URL
        return typed(raw, http_config, **(changes or {}))
    return cell.retrieve_cellosaurus_glioblastoma_cell_line_metadata(approve_source_dispatch=True, retrieved_at=NOW, fetcher=fetch)


def rows(value):
    return value["Cellosaurus"]["cell-line-list"]


def resolve(value, pointer):
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def restamp(value):
    value["bundle_digest"] = content_digest({key: item for key, item in value.items() if key != "bundle_digest"})
    return value


def test_default_reader_uses_exact_existing_get_wire_once_without_source_link_or_retry():
    raw = body(native())
    config = cell._http_config(cell.CellosaurusCellLineRetrievalConfig())
    assert config.config_digest == "0b7a2a158eb53a8a241cc95654286b7614b868e27708a7ac4852f4fde3f9252a"
    with transport([response_wire(raw)], configuration=config) as (_client, _raw, secured, dns, _context, _socket):
        value = cell.retrieve_cellosaurus_glioblastoma_cell_line_metadata(approve_source_dispatch=True, retrieved_at=NOW)
    assert value["transport"]["mode"] == "default_reviewed_public_http"
    assert value["transport"]["timing_authority"] == "builtin_total_deadline"
    assert dns.call_count == 1
    sent = bytes(secured.sent)
    assert sent.startswith(b"GET /search/cell-line?q=ox%3Asapiens+din%3Aglioblastoma&start=0&rows=5&format=json&fields=ac%2Cid%2Csy%2Cox%2Cdin%2Ccaution%2Cproblematic%2Cdt&sort=ac+asc HTTP/1.1\r\nHost: api.cellosaurus.org\r\n")
    assert sent.endswith(b"\r\n\r\n") and value["receipts"][0]["source_request_count"] == 1


def test_exact_raw_bytes_selected_array_order_strings_and_all_native_citation_values_replay(monkeypatch):
    source = native(2)
    rows(source)[0]["name-list"][0]["value"] = "synthetic α alias"
    raw = body(source) + b"\n "
    value = retrieve(raw=raw)
    retained = value["source_inputs"][0]
    assert base64.b64decode(retained["response_body_base64"], validate=True) == raw
    assert retained["response_digest"] == hashlib.sha256(raw).hexdigest()
    assert retained["native_payload"] == source
    assert value["records"][0]["observations"]["name-list"]["value"] == rows(source)[0]["name-list"]
    assert value["records"][0]["identifier"] == "synthetic identity 0"
    assert value["records"][0]["observations"]["entry-version"]["value"] == "00050"
    for citation in value["citation_index"]:
        assert citation["artifact_pointer_root"] == "validated_cellosaurus_bundle"
        observed = resolve(value, citation["artifact_pointer"])
        assert observed == citation["source_value"] == resolve(source, citation["native_payload_pointer"])
        assert citation["source_value_digest"] == content_digest(observed)
        assert citation["citation_id"] == "cellosaurus_native:" + content_digest({k: v for k, v in citation.items() if k != "citation_id"})
        assert citation["retrieved_at"] == NOW
    monkeypatch.setattr(wire.ReviewedPublicHttpClient, "get", Mock(side_effect=AssertionError("replay dispatched")))
    assert cell.validate_cellosaurus_cell_line_bundle(value) == value


@pytest.mark.parametrize("count", [0, 1, 5])
def test_empty_short_and_full_windows_never_invent_total_completeness_or_denominators(count):
    value = retrieve(native(count))
    coverage = value["coverage"]
    assert len(value["records"]) == coverage["returned_record_count"] == count
    assert coverage["reported_total_count"] is coverage["unreturned_count"] is coverage["page_count"] is None
    assert coverage["completeness"] == coverage["omitted_influence"] == "unknown"
    assert coverage["pagination_followed"] is False
    assert coverage["source_snapshot_identity"] == "unestablished"
    assert coverage["patient_or_donor_count"] is coverage["cohort_or_assay_denominator"] is coverage["independent_publication_count"] is None
    assert all(row["omitted_count"] is None and row["influence"] == "unknown" for row in value["omissions"])
    assert value["scientific_support_established"] is value["clinical_actions_authorized"] is value["laboratory_actions_authorized"] is False


@pytest.mark.parametrize("field", ["comment-list", "created", "last-updated", "entry-version"])
def test_synthetic_optional_missing_and_null_remain_distinct_with_containing_absence_citation(field):
    missing, null = native(), native()
    del rows(missing)[0][field]
    rows(null)[0][field] = None
    a, b = retrieve(missing), retrieve(null)
    absent, explicit = a["records"][0]["observations"][field], b["records"][0]["observations"][field]
    assert absent["state"] == "missing" and absent["native_pointer"] is None and absent["value_citation_id"] is None
    assert absent["absence_context_citation_id"] == a["records"][0]["citation_id"]
    assert explicit["state"] == "null" and explicit["native_pointer"].endswith("/" + field)
    assert explicit["value"] is None and explicit["value_citation_id"] and explicit["absence_context_citation_id"] is None
    assert a["records"][0]["problem_absence_established"] is b["records"][0]["problem_absence_established"] is False


def test_synthetic_empty_comments_and_disease_list_do_not_become_problem_absence_or_confirmed_query_classification():
    source = native()
    rows(source)[0]["comment-list"] = []
    rows(source)[0]["disease-list"] = []
    value = retrieve(source)
    record = value["records"][0]
    assert record["observations"]["comment-list"]["state"] == "observed"
    assert record["observations"]["comment-list"]["value"] == []
    assert record["array_entries"]["disease-list"] == []
    assert record["source_query_disease_correspondence"] == "unestablished"
    assert record["problem_absence_established"] is record["authenticity_or_quality_or_suitability_assessed"] is False


@pytest.mark.parametrize("field", ["species-list", "disease-list"])
@pytest.mark.parametrize("optional", ["category", "iri", "url"])
def test_optional_reference_null_missing_and_inert_links_are_retained_with_absence_context(field, optional):
    missing, null = native(), native()
    del rows(missing)[0][field][0][optional]
    rows(null)[0][field][0][optional] = None
    a, b = retrieve(missing), retrieve(null)
    absent = a["records"][0]["array_entries"][field][0]
    assert absent["observations"][optional]["state"] == "missing"
    assert absent["observations"][optional]["absence_context_citation_id"] == absent["citation_id"]
    assert b["records"][0]["array_entries"][field][0]["observations"][optional]["state"] == "null"


def test_all_species_and_disease_assertions_retained_without_human_only_or_ontology_equivalence_claim():
    source = native()
    row = rows(source)[0]
    row["species-list"].append(reference("NCBI_TaxID", "10090", "Synthetic second taxonomic assertion"))
    row["disease-list"] = [reference("NCIt", "C39750", "Glioblastoma, IDH-wildtype"), reference("NCIt", "C3058", "Glioblastoma")]
    value = retrieve(source)
    record = value["records"][0]
    assert record["observations"]["species-list"]["value"] == row["species-list"]
    assert record["observations"]["disease-list"]["value"] == row["disease-list"]
    assert record["source_identity_equivalence"] == record["source_query_disease_correspondence"] == "unestablished"


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_approval_is_required_before_even_a_caller_fetcher(approval):
    fetcher = Mock(side_effect=AssertionError("approval bypass"))
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="approval"):
        cell.retrieve_cellosaurus_glioblastoma_cell_line_metadata(approve_source_dispatch=approval, fetcher=fetcher)
    fetcher.assert_not_called()


@pytest.mark.parametrize("timeout", [True, None, 99, 30001, 100.0, "30000"])
def test_timeout_is_exact_bounded_integer_without_type_coercion(timeout):
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        cell.CellosaurusCellLineRetrievalConfig(timeout)


@pytest.mark.parametrize("field", sorted(cell._REQUIRED))
@pytest.mark.parametrize("state", ["missing", "null"])
def test_required_identity_and_query_metadata_missing_or_null_refuse_whole_window(field, state):
    source = native()
    if state == "missing": del rows(source)[0][field]
    else: rows(source)[0][field] = None
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("change", ["second_primary", "secondary", "primary_missing", "identifier_missing", "two_identifiers",
    "unknown_name_type", "duplicate_primary", "descending", "taxonomy_missing", "wrong_taxonomy_database", "wrong_disease_database",
    "unknown_comment", "guessed_caution", "extra_record_field", "extra_reference_field", "extra_wrapper", "too_many_records",
    "integer_version", "empty_label", "comment_missing_value", "null_name", "null_comment_entry"])
def test_ambiguous_identity_unknown_shapes_and_scope_changes_refuse_without_sorting_or_alias_fallback(change):
    source = native(2)
    row = rows(source)[0]
    if change == "second_primary": row["accession-list"] *= 2
    elif change == "secondary": row["accession-list"][0]["type"] = "secondary"
    elif change == "primary_missing": row["accession-list"] = []
    elif change == "identifier_missing": row["name-list"].pop()
    elif change == "two_identifiers": row["name-list"][0]["type"] = "identifier"
    elif change == "unknown_name_type": row["name-list"][0]["type"] = "preferred"
    elif change == "duplicate_primary": rows(source)[1]["accession-list"] = copy.deepcopy(row["accession-list"])
    elif change == "descending": rows(source).reverse()
    elif change == "taxonomy_missing": row["species-list"][0]["accession"] = "10090"
    elif change == "wrong_taxonomy_database": row["species-list"][0]["database"] = "NCIt"
    elif change == "wrong_disease_database": row["disease-list"][0]["database"] = "Other"
    elif change == "unknown_comment": row["comment-list"][0]["category"] = "Quality passed"
    elif change == "guessed_caution": row["comment-list"][0]["category"] = "Caution"
    elif change == "extra_record_field": row["culture-medium"] = "excluded"
    elif change == "extra_reference_field": row["disease-list"][0]["extra"] = "excluded"
    elif change == "extra_wrapper": source["Cellosaurus"]["count"] = 2
    elif change == "too_many_records": source = native(6)
    elif change == "integer_version": row["entry-version"] = 50
    elif change == "empty_label": row["disease-list"][0]["label"] = ""
    elif change == "comment_missing_value": del row["comment-list"][0]["value"]
    elif change == "null_name": row["name-list"][0] = None
    elif change == "null_comment_entry": row["comment-list"][0] = None
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("raw", [b'{"Cellosaurus":{},"Cellosaurus":{}}', b'{"Cellosaurus":NaN}', b"{", b"[]", b"null", b""])
def test_duplicate_nonfinite_or_malformed_json_never_becomes_an_empty_page(raw):
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(raw=raw)


def test_bounded_parser_refuses_depth_and_node_overflow_before_native_projection():
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="json_tree_bound"):
        cell._parse(b"[" * 21 + b"0" + b"]" * 21)
    raw = json.dumps({"synthetic": [None] * 30000}).encode()
    assert len(raw) < cell.MAX_CELLOSAURUS_RESPONSE_BYTES
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="json_tree_bound"):
        cell._parse(raw)


@pytest.mark.parametrize("field,maximum", [("name-list", 128), ("species-list", 16), ("disease-list", 32), ("comment-list", 32)])
def test_native_array_caps_refuse_entire_window_instead_of_prefix_selection(field, maximum):
    source = native()
    row = rows(source)[0]
    entry = copy.deepcopy(row[field][0])
    row[field] = [entry] * (maximum + 1)
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(source)


@pytest.mark.parametrize("field,maximum", [("identifier", 4096), ("accession", 128), ("comment", 32768), ("date", 128), ("link", 4096)])
def test_utf8_string_bounds_are_measured_in_bytes_and_not_truncated(field, maximum):
    source = native()
    row = rows(source)[0]
    value = "α" * (maximum // 2 + 1)
    if field == "identifier": row["name-list"][-1]["value"] = value
    elif field == "accession": row["accession-list"][0]["value"] = value
    elif field == "comment": row["comment-list"][0]["value"] = value
    elif field == "date": row["created"] = value
    else: row["disease-list"][0]["iri"] = value
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(source)


def test_response_byte_bound_allows_exact_raw_limit_and_refuses_one_more_byte():
    raw = body(native())
    padded = raw + b" " * (cell.MAX_CELLOSAURUS_RESPONSE_BYTES - len(raw))
    assert retrieve(raw=padded)["source_inputs"][0]["response_bytes"] == cell.MAX_CELLOSAURUS_RESPONSE_BYTES
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(raw=padded + b" ")


def test_full_allowed_array_shapes_can_refuse_whole_citation_bound_without_dropping_any_member():
    source = native(5)
    for row in rows(source):
        row["name-list"] = [{"type": "synonym", "value": "s"}] * 127 + [{"type": "identifier", "value": "i"}]
        row["species-list"] *= 16
        row["disease-list"] *= 32
        row["comment-list"] *= 32
    assert len(body(source)) < cell.MAX_CELLOSAURUS_RESPONSE_BYTES
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="citation_bound"):
        retrieve(source)


def test_whole_artifact_byte_bound_is_independent_of_body_and_citation_caps(monkeypatch):
    monkeypatch.setattr(cell, "MAX_CELLOSAURUS_BUNDLE_BYTES", 1)
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve()


@pytest.mark.parametrize("change", ["projection", "array_order", "payload", "raw", "query", "coverage", "citation", "citation_id",
    "absence_context", "receipt", "clock_invalid", "typed_metadata", "default_mode", "base64", "additional_field"])
def test_recomputed_outer_digest_cannot_conceal_disagreement_with_complete_raw_rebuild(change):
    value = retrieve()
    if change == "projection": value["records"][0]["identifier"] = "replacement"
    elif change == "array_order": value["records"][0]["array_entries"]["name-list"].reverse()
    elif change == "payload": value["source_inputs"][0]["native_payload"]["Cellosaurus"]["cell-line-list"][0]["entry-version"] = "changed"
    elif change == "raw": value["source_inputs"][0]["response_body_base64"] = base64.b64encode(body(native(0))).decode()
    elif change == "query": value["query"]["rows"] = 1
    elif change == "coverage": value["coverage"]["completeness"] = "complete"
    elif change == "citation": value["citation_index"][0]["native_payload_pointer"] += "/0"
    elif change == "citation_id": value["citation_index"][0]["citation_id"] = "restamped"
    elif change == "absence_context": value["records"][0]["observations"]["created"]["absence_context_citation_id"] = "invented"
    elif change == "receipt": value["receipts"][0]["source_request_count"] = 0
    elif change == "clock_invalid": value["retrieved_at"] = "yesterday"
    elif change == "typed_metadata": value["source_inputs"][0]["typed_transport_metadata"]["config_digest"] = "0" * 64
    elif change == "default_mode": value["transport"]["mode"] = "default_reviewed_public_http"
    elif change == "base64": value["source_inputs"][0]["response_body_base64"] += "="
    elif change == "additional_field": value["laboratory_actions_authorized"] = True
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        cell.validate_cellosaurus_cell_line_bundle(restamp(value))


def test_coherent_whole_raw_substitution_remains_an_explicit_caller_assertion_not_authentication():
    value = retrieve(native())
    replacement = cell._bundle(cell.CellosaurusCellLineRetrievalConfig(), body=body(native(0)), retrieved_at=NOW,
        mode="caller_supplied", binding=value["transport"]["binding_id"])
    assert cell.validate_cellosaurus_cell_line_bundle(replacement) == replacement
    assert replacement["source_origin_authentication"] == "unestablished"


@pytest.mark.parametrize("changes", [{"config_digest": "0" * 64}, {"transport_policy_digest": "0" * 64},
    {"request_url": "https://api.cellosaurus.org/other"}, {"content_type": "application/problem+json"}])
def test_wrong_typed_transport_scope_policy_or_content_type_is_refused(changes):
    with pytest.raises(cell.CellosaurusCellLineRetrievalError):
        retrieve(changes=changes)


def test_caller_exception_and_late_response_are_refused_without_retry_or_empty_fallback(monkeypatch):
    fetcher = Mock(side_effect=RuntimeError("private fixture detail"))
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="source_request_failed") as caught:
        cell.retrieve_cellosaurus_glioblastoma_cell_line_metadata(approve_source_dispatch=True, fetcher=fetcher)
    assert "private fixture detail" not in str(caught.value) and fetcher.call_count == 1
    ticks = iter([0.0, 31.0])
    monkeypatch.setattr(cell, "monotonic", lambda: next(ticks))
    with pytest.raises(cell.CellosaurusCellLineRetrievalError, match="deadline"):
        retrieve()


def test_explicit_registration_is_atomic_and_caller_binding_is_not_a_default_live_descriptor():
    a, b = WorkflowOperationCatalog(), WorkflowOperationCatalog()
    cell.register_cellosaurus_cell_line_operations(a, fetcher=lambda *_args, **_kwargs: None)
    cell.register_cellosaurus_cell_line_operations(b, fetcher=lambda *_args, **_kwargs: None)
    assert a.catalog_digest != b.catalog_digest
    descriptor = a.resolve(cell.CELLOSAURUS_CELL_LINE_OPERATION_ID)
    assert descriptor.implementation == "caller_adapter" and descriptor.read_only is True
    assert set(descriptor.input_schema["properties"]) == {"timeout_ms"} and descriptor.input_schema["additionalProperties"] is False
    before = a.to_dict()
    with pytest.raises(ArgumentError):
        cell.register_cellosaurus_cell_line_operations(a)
    assert a.to_dict() == before


def test_onboarded_reader_keeps_default_catalogue_and_has_explicit_current_registry_inventory():
    from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
    from prism_sdk.workflow_provider_registry import reviewed_workflow_provider_registry
    base = builtin_workflow_operation_catalog()
    assert base.catalog_digest == "53e11ef585fa6d65736ecc77d5042962a9276fac8fbe7b39bde1efccfec5e5b2"
    counts = reviewed_workflow_provider_registry(base).to_dict()["counts"]
    assert counts["implemented_reader_paths"] == 20 and counts["declared_reviewed_service_groups"] == 17
