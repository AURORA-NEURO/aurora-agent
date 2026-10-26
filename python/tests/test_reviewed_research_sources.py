from __future__ import annotations

from copy import deepcopy
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import threading
from urllib.parse import parse_qs, urlsplit

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.reviewed_research_sources import (
    MAX_RESEARCH_SOURCE_RESPONSE_BYTES, RESEARCH_SOURCE_TIMEOUT_SECONDS,
    ResearchSourceHttpResponse, ResearchSourceRetrievalError,
    execute_research_source_query, prepare_research_source_query,
    register_research_source_operations,
)
from prism_sdk import reviewed_research_sources as sources


_AT = "2026-10-26T15:00:00Z"
_IDS = {"openalex": "openalex.search_works", "crossref": "crossref.search_works", "zenodo": "zenodo.search_records"}


def _payload(source):
    if source == "openalex":
        return {"meta": {"count": 7, "page": 1, "per_page": 2}, "results": [
            {"id": "https://openalex.org/W123456", "doi": "https://doi.org/10.1000/Glioma-A", "display_name": "Glioma metadata A", "publication_year": 2024, "publication_date": "2024-02-01", "type": "article", "abstract_inverted_index": {"UNPROJECTED_PRIVATE_TEXT": [0]}},
            {"id": "https://openalex.org/W123457", "doi": None, "display_name": "Glioma metadata B", "publication_year": None, "publication_date": None, "type": None},
        ]}
    if source == "crossref":
        return {"status": "ok", "message-type": "work-list", "message": {"total-results": 7, "items-per-page": 2, "items": [
            {"DOI": "10.1000/Glioma-A", "title": ["Glioma metadata A"], "issued": {"date-parts": [[2024, 2, 1]]}, "type": "journal-article", "abstract": "UNPROJECTED_PRIVATE_TEXT", "author": [{"email": "UNPROJECTED_PRIVATE_TEXT"}]},
            {"DOI": "10.1000/Glioma-B", "title": [], "issued": {"date-parts": [[2023]]}, "type": None},
        ]}}
    return {"hits": {"total": {"value": 7, "relation": "gte"}, "hits": [
        {"id": 123456, "doi": "10.5281/zenodo.123456", "metadata": {"title": "Glioma metadata A", "publication_date": "2024-02-01", "resource_type": {"type": "dataset"}, "description": "UNPROJECTED_PRIVATE_TEXT"}, "files": [{"key": "UNPROJECTED_PRIVATE_TEXT", "links": {"self": "https://unrequested.invalid/private-file"}}]},
        {"id": "123457", "pids": {"doi": {"identifier": "10.5281/zenodo.123457"}}, "metadata": {"title": "Glioma metadata B"}},
    ]}}


def _fetcher(payload, calls, **overrides):
    encoded = payload if isinstance(payload, bytes) else json.dumps(payload).encode()

    def fetch(url, *, timeout_seconds, max_response_bytes):
        calls.append({"url": url, "timeout": timeout_seconds, "max_bytes": max_response_bytes})
        return ResearchSourceHttpResponse(overrides.get("status", 200), encoded, overrides.get("final_url", url), overrides.get("content_type", "application/json"))

    return fetch


@pytest.mark.parametrize("source", list(_IDS))
def test_pure_plans_and_registration_do_not_fetch_or_accept_model_owned_endpoints(source):
    calls = []
    fetch = _fetcher(_payload(source), calls)
    catalog = register_research_source_operations(WorkflowOperationCatalog(), fetchers={source: fetch})
    plan = prepare_research_source_query(source, query="glioma", limit=2)
    assert plan.to_dict() == prepare_research_source_query(source, query="glioma", limit=2).to_dict()
    assert plan.to_dict()["maximum_requests"] == 1
    assert "glioma" not in json.dumps(plan.to_dict())
    assert catalog.to_dict()["integration_count"] == 3
    assert calls == []
    for approval in (False, None, 1, "true"):
        with pytest.raises(ValueError):
            catalog.execute(_IDS[source], {"query": "glioma", "limit": 2}, approve_source_dispatch=approval)
        with pytest.raises(ResearchSourceRetrievalError, match="approval"):
            execute_research_source_query(plan, approve_source_dispatch=approval, fetcher=fetch)
    assert calls == []
    for arguments in ({"query": "glioma", "limit": 26}, {"url": "https://unrequested.invalid"}, {"headers": {"Accept": "anything"}}, {"query": "glioma", "api_key": "UNPROJECTED_PRIVATE_TEXT"}):
        with pytest.raises(ValueError):
            catalog.execute(_IDS[source], arguments, approve_source_dispatch=True)
    assert calls == []


@pytest.mark.parametrize("source", list(_IDS))
def test_real_request_and_parser_contract_projects_only_citable_identity_metadata(source):
    calls = []
    payload = _payload(source)
    catalog = register_research_source_operations(WorkflowOperationCatalog(), fetchers={source: _fetcher(payload, calls)})
    result = catalog.execute(_IDS[source], {"query": "glioma", "limit": 2}, approve_source_dispatch=True, retrieved_at=_AT)
    bundle, receipt = result.value, result.receipt
    assert len(calls) == 1
    parsed = urlsplit(calls[0]["url"])
    assert parsed.scheme == "https"
    parameters = parse_qs(parsed.query)
    if source == "openalex":
        assert parsed.netloc == "api.openalex.org"
        assert parameters["search"] == ["glioma"]
        assert parameters["page"] == ["1"] and parameters["per_page"] == ["2"]
        assert "abstract" not in parameters["select"][0]
    elif source == "crossref":
        assert parsed.netloc == "api.crossref.org"
        assert parameters["query.bibliographic"] == ["glioma"]
        assert parameters["rows"] == ["2"] and parameters["offset"] == ["0"]
        assert parameters["select"] == ["DOI,title,issued,type"]
    else:
        assert parsed.netloc == "zenodo.org"
        assert parameters["q"] == ['"glioma"']
        assert parameters["page"] == ["1"] and parameters["size"] == ["2"]
    assert calls[0]["timeout"] == RESEARCH_SOURCE_TIMEOUT_SECONDS
    assert calls[0]["max_bytes"] == MAX_RESEARCH_SOURCE_RESPONSE_BYTES
    assert bundle["returned_count"] == 2
    assert bundle["reported_total_count"] == 7
    assert bundle["truncated"] is True and bundle["coverage"] == "partial"
    assert bundle["omitted_count"] == (None if source == "zenodo" else 5)
    assert bundle["total_count_relation"] == ("lower_bound" if source == "zenodo" else "exact")
    assert bundle["records"][0]["citation_url"].startswith("https://doi.org/")
    assert "UNPROJECTED_PRIVATE_TEXT" not in json.dumps(result.value)
    assert "UNPROJECTED_PRIVATE_TEXT" not in json.dumps(receipt)
    assert "Glioma metadata" not in json.dumps(receipt)
    assert receipt["network"] == "caller_transport" and receipt["request_count"] == 1
    assert bundle["bundle_digest"] == content_digest({key: value for key, value in bundle.items() if key != "bundle_digest"})
    assert receipt["receipt_digest"] == content_digest({key: value for key, value in receipt.items() if key != "receipt_digest"})
    assert receipt["response_digest"] == hashlib.sha256(json.dumps(payload).encode()).hexdigest()
    assert bundle["records"][1]["missing_fields"]


@pytest.mark.parametrize("reported,relation,truncated,coverage,omitted", [
    (None, "unknown", None, "unknown", None),
    (0, "exact", False, "complete", 0),
    ({"value": 0, "relation": "gte"}, "lower_bound", None, "unknown", None),
    ({"value": 100, "relation": "gte"}, "lower_bound", True, "partial", None),
])
def test_missing_totals_zero_results_and_lower_bounds_remain_distinct(reported, relation, truncated, coverage, omitted):
    payload = {"hits": {"hits": []}}
    if reported is not None:
        payload["hits"]["total"] = reported
    calls = []
    result = execute_research_source_query(prepare_research_source_query("zenodo", limit=2), approve_source_dispatch=True, fetcher=_fetcher(payload, calls), retrieved_at=_AT)
    assert result.value["returned_count"] == 0
    assert result.value["reported_total_count"] == (reported["value"] if isinstance(reported, dict) else reported)
    assert result.value["total_count_relation"] == relation
    assert result.value["truncated"] is truncated
    assert result.value["coverage"] == coverage
    assert result.value["omitted_count"] == omitted


@pytest.mark.parametrize("parts", [[[None]], [[]]])
def test_crossref_unknown_issued_date_preserves_citable_record_and_explicit_missingness(parts):
    payload = _payload("crossref")
    payload["message"]["items"][0]["issued"]["date-parts"] = parts
    result = execute_research_source_query(prepare_research_source_query("crossref", limit=2), approve_source_dispatch=True, fetcher=_fetcher(payload, []), retrieved_at=_AT)
    record = result.value["records"][0]
    assert record["doi"] == "10.1000/glioma-a"
    assert record["publication_date"] is record["publication_year"] is None
    assert "publication_date" in record["missing_fields"] and "publication_year" in record["missing_fields"]


def test_zenodo_doi_aliases_preserve_known_identity_and_refuse_conflicting_records():
    payload = _payload("zenodo")
    payload["hits"]["hits"][0]["doi"] = None
    payload["hits"]["hits"][0]["metadata"]["doi"] = "10.5281/zenodo.123456"
    plan = prepare_research_source_query("zenodo", limit=2)
    result = execute_research_source_query(plan, approve_source_dispatch=True, fetcher=_fetcher(payload, []), retrieved_at=_AT)
    assert result.value["records"][0]["doi"] == "10.5281/zenodo.123456"
    payload["hits"]["hits"][0]["pids"] = {"doi": {"identifier": "10.5281/zenodo.999999"}}
    with pytest.raises(ResearchSourceRetrievalError, match="doi_mismatch"):
        execute_research_source_query(plan, approve_source_dispatch=True, fetcher=_fetcher(payload, []), retrieved_at=_AT)


@pytest.mark.parametrize("query", ["", "   ", "query:glioma", "https://private.invalid/path", "glioma&api_key=secret", 'glioma" OR *', "glioma\nmutation", "g" * 201])
def test_query_grammar_cannot_escape_fixed_public_request_scope(query):
    with pytest.raises(ResearchSourceRetrievalError):
        prepare_research_source_query("zenodo", query=query)


@pytest.mark.parametrize("mutate,code", [
    (lambda value: value["meta"].update(page=2), "page_mismatch"),
    (lambda value: value["meta"].update(count=True), "total_count_invalid"),
    (lambda value: value["meta"].update(count=1), "below_returned"),
    (lambda value: value["results"].append(deepcopy(value["results"][0])), "collection_bound"),
    (lambda value: value["results"][1].update(id=value["results"][0]["id"]), "duplicate_record"),
    (lambda value: value["results"][0].update(id="https://unrequested.invalid/W123"), "identity_invalid"),
    (lambda value: value["results"][0].update(publication_year=2023), "year_mismatch"),
    (lambda value: value["results"][0].update(doi="https://unrequested.invalid/metadata"), "doi_invalid"),
])
def test_source_contract_drift_and_duplicate_identity_fail_closed(mutate, code):
    payload = _payload("openalex")
    mutate(payload)
    with pytest.raises(ResearchSourceRetrievalError, match=code):
        execute_research_source_query(prepare_research_source_query("openalex", limit=2), approve_source_dispatch=True, fetcher=_fetcher(payload, []), retrieved_at=_AT)


@pytest.mark.parametrize("options,code", [
    ({"status": 302}, "redirect_refused"),
    ({"status": 429}, "rate_limited"),
    ({"status": 403}, "http_error"),
    ({"final_url": "https://unrequested.invalid/records"}, "redirect_refused"),
    ({"content_type": "text/html"}, "json_content_type_required"),
])
def test_transport_refusals_never_parse_or_publish_error_bodies(options, code):
    with pytest.raises(ResearchSourceRetrievalError, match=code) as caught:
        execute_research_source_query(prepare_research_source_query("openalex", limit=2), approve_source_dispatch=True, fetcher=_fetcher(b"UNPROJECTED_PRIVATE_TEXT", [], **options), retrieved_at=_AT)
    assert "UNPROJECTED_PRIVATE_TEXT" not in str(caught.value)


@pytest.mark.parametrize("body,code", [
    (b'{"meta":{"count":0,"count":1},"results":[]}', "duplicate_json"),
    (b'{"meta":{"count":NaN},"results":[]}', "non_finite"),
    (b'{"extra":1e999,"meta":{"count":0},"results":[]}', "non_finite"),
    (b"not-json-UNPROJECTED_PRIVATE_TEXT", "invalid_json"),
    (b"x" * (MAX_RESEARCH_SOURCE_RESPONSE_BYTES + 1), "byte_bound"),
], ids=["duplicate_fields", "nan", "infinite_exponent", "invalid_json", "oversize_bytes"])
def test_json_and_size_bounds_reject_unchecked_payloads_without_echoing_content(body, code):
    with pytest.raises(ResearchSourceRetrievalError, match=code) as caught:
        execute_research_source_query(prepare_research_source_query("openalex", limit=2), approve_source_dispatch=True, fetcher=_fetcher(body, []), retrieved_at=_AT)
    assert "UNPROJECTED_PRIVATE_TEXT" not in str(caught.value)


def test_tree_bound_and_invalid_timestamp_fail_before_any_record_projection_or_network():
    nested = {}
    for _ in range(40):
        nested = {"nested": nested}
    payload = {"meta": {"count": 0}, "results": [], "ignored": nested}
    with pytest.raises(ResearchSourceRetrievalError, match="tree_bound"):
        execute_research_source_query(prepare_research_source_query("openalex"), approve_source_dispatch=True, fetcher=_fetcher(payload, []), retrieved_at=_AT)
    calls = []
    with pytest.raises(ResearchSourceRetrievalError, match="whole_seconds"):
        execute_research_source_query(prepare_research_source_query("openalex"), approve_source_dispatch=True, fetcher=_fetcher(payload, calls), retrieved_at="2026-10-26T15:00:00.123Z")
    assert calls == []


def test_caller_transport_exception_text_is_not_retained_or_chained():
    def broken(*_args, **_kwargs):
        raise RuntimeError("UNPROJECTED_PRIVATE_TEXT")

    with pytest.raises(ResearchSourceRetrievalError, match="caller_transport_error") as caught:
        execute_research_source_query(prepare_research_source_query("openalex"), approve_source_dispatch=True, fetcher=broken)
    assert "UNPROJECTED_PRIVATE_TEXT" not in str(caught.value)
    assert caught.value.__cause__ is None and caught.value.__suppress_context__ is True


def test_builtin_transport_enforces_fixed_headers_timeout_bytes_and_no_redirect_handler(monkeypatch):
    plan = prepare_research_source_query("openalex", limit=2)
    body = json.dumps(_payload("openalex")).encode()
    calls = []

    class Response:
        status = 200
        headers = {"Content-Type": "application/json", "Content-Length": str(len(body))}

        def __init__(self):
            self.offset = 0

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return None

        def geturl(self):
            return plan.request_url

        def read(self, maximum):
            calls.append(("read", maximum))
            chunk = body[self.offset:self.offset + maximum]
            self.offset += len(chunk)
            return chunk

        read1 = read

    class Opener:
        def open(self, request, *, timeout):
            calls.append((request.full_url, request.get_method(), dict(request.header_items()), timeout))
            return Response()

    def opener_factory(handler):
        assert isinstance(handler, sources._NoRedirect)
        assert handler.redirect_request(None) is None
        return Opener()

    monkeypatch.setattr(sources, "build_opener", opener_factory)
    monkeypatch.setattr(sources, "_pace", lambda source_id: calls.append(("pace", source_id)))
    result = execute_research_source_query(plan, approve_source_dispatch=True, retrieved_at=_AT)
    request = next(row for row in calls if len(row) == 4)
    assert request[0] == plan.request_url and request[1] == "GET"
    assert request[3] == RESEARCH_SOURCE_TIMEOUT_SECONDS
    assert set(name.lower() for name in request[2]) == {"accept", "user-agent"}
    assert result.receipt["network"] == "builtin_https"
    assert max(row[1] for row in calls if row[0] == "read") <= 65_536


def test_registration_preflights_duplicates_and_invalid_fetchers_without_partial_mutation():
    catalog = WorkflowOperationCatalog()
    digest = catalog.catalog_digest
    with pytest.raises(ResearchSourceRetrievalError):
        register_research_source_operations(catalog, fetchers={"unreviewed": lambda *_args: None})
    assert catalog.catalog_digest == digest
    assert register_research_source_operations(catalog) is catalog
    registered = catalog.catalog_digest
    with pytest.raises(ResearchSourceRetrievalError, match="already_registered"):
        register_research_source_operations(catalog)
    assert catalog.catalog_digest == registered


def test_builtin_source_requests_remain_serial_even_when_a_prior_response_is_slow(monkeypatch):
    entered = threading.Event()
    release = threading.Event()
    calls = []
    payload = _payload("crossref")

    def fetch(url, **kwargs):
        calls.append(url)
        if len(calls) == 1:
            entered.set()
            assert release.wait(3)
        return ResearchSourceHttpResponse(200, json.dumps(payload).encode(), url)

    monkeypatch.setattr(sources, "_builtin_fetch", fetch)
    monkeypatch.setattr(sources, "_pace", lambda _source: None)
    plan = prepare_research_source_query("crossref", limit=2)
    with ThreadPoolExecutor(max_workers=2) as pool:
        first = pool.submit(execute_research_source_query, plan, approve_source_dispatch=True, retrieved_at=_AT)
        assert entered.wait(3)
        second_started = threading.Event()

        def second_request():
            second_started.set()
            return execute_research_source_query(plan, approve_source_dispatch=True, retrieved_at=_AT)

        second = pool.submit(second_request)
        assert second_started.wait(3)
        assert len(calls) == 1
        release.set()
        assert first.result(timeout=3).value["returned_count"] == 2
        assert second.result(timeout=3).value["returned_count"] == 2
    assert len(calls) == 2
