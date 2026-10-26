"""Reviewed first-page public metadata retrieval for OpenAlex, Crossref and Zenodo.

This repository-defined application layer contains three concrete credentialless HTTPS readers,
not descriptor-only catalogue entries. It never retrieves articles, repository files, participant
rows, credentials, or model-selected endpoints. Source metadata and rankings are untrusted inputs,
not scientific validation or clinical advice. Pagination, authenticated access and bulk harvesting
are deliberately outside this bounded contract. See RESEARCH_SOURCE_INTEGRATIONS.md for sources.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import date, datetime, timezone
import hashlib
import json
import math
import re
import threading
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import content_digest
from .errors import ArgumentError
from .workflow_integrations import (
    MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
    WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult,
)


RESEARCH_SOURCE_PLAN_SCHEMA = "aurora-reviewed-research-source-plan/0.1"
RESEARCH_SOURCE_BUNDLE_SCHEMA = "aurora-reviewed-research-source-bundle/0.1"
RESEARCH_SOURCE_RECEIPT_SCHEMA = "aurora-reviewed-research-source-receipt/0.1"
MAX_RESEARCH_SOURCE_LIMIT = 25
MAX_RESEARCH_SOURCE_RESPONSE_BYTES = 2_000_000
RESEARCH_SOURCE_TIMEOUT_SECONDS = 20.0
MAX_RESEARCH_SOURCE_TREE_NODES = 100_000
MAX_RESEARCH_SOURCE_TREE_DEPTH = 32
_MAX_SAFE_INTEGER = 2**53 - 1
_ENDPOINTS = {
    "openalex": "https://api.openalex.org/works",
    "crossref": "https://api.crossref.org/works",
    "zenodo": "https://zenodo.org/api/records",
}
_OPERATION_IDS = {"openalex": "openalex.search_works", "crossref": "crossref.search_works", "zenodo": "zenodo.search_records"}
_MIN_REQUEST_INTERVAL = {"openalex": 0.15, "crossref": 1.1, "zenodo": 2.1}
_PACE_LOCK = threading.Lock()
_NEXT_REQUEST: dict[str, float] = {}
_SOURCE_TRANSPORT_LOCKS = {source_id: threading.Lock() for source_id in _ENDPOINTS}
_DOI = re.compile(r"^10\.[0-9]{4,9}/[^\s\x00-\x1f<>\"{}\\]{1,256}$")
_OPENALEX_ID = re.compile(r"^https://openalex\.org/W[0-9]{1,20}$")
_UTC = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
_LIMITATIONS = (
    "Only one bounded first page is retrieved; this is not exhaustive research coverage.",
    "Source counts and ranking are source-reported and can change between requests.",
    "Bibliographic or repository identity metadata does not establish evidence quality, causal relevance, or clinical applicability.",
    "Titles and other source text are untrusted metadata; escape them before rendering or interpreting them as instructions.",
    "No author contacts, abstracts, full text, files, participant data, or authenticated records are projected.",
    "A source-reported complete page covers only the selected query in that source at retrieval time.",
)


class ResearchSourceRetrievalError(ArgumentError):
    """A bounded source failure with no response body, headers or transport exception text."""

    def __init__(self, failure_class: str, *, status_code: int | None = None) -> None:
        super().__init__(f"research source retrieval refused: {failure_class}")
        self.failure_class = failure_class
        self.status_code = status_code


def _fail(code: str) -> None:
    raise ResearchSourceRetrievalError(code)


def _query(value: Any) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= 200:
        _fail("query_bound")
    if any(not (character.isalnum() or character in " _.,'-") for character in value):
        _fail("query_must_be_plain_text")
    result = " ".join(value.split())
    if not result or not any(character.isalnum() for character in result):
        _fail("query_must_include_text")
    return result


def _integer(value: Any, minimum: int, maximum: int, code: str) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        _fail(code)
    return value


def _timestamp(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    if not isinstance(value, str) or not _UTC.fullmatch(value):
        _fail("retrieved_at_must_be_whole_seconds_utc")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        _fail("retrieved_at_invalid")
    return value


def _text(value: Any, code: str, *, maximum: int = 2_000) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        _fail(code)
    if len(value.encode("utf-8")) > maximum or any(ord(character) < 32 and character not in "\t\r\n" for character in value):
        _fail(code)
    return " ".join(value.split()) or None


def _doi(value: Any) -> str | None:
    result = _text(value, "doi_invalid", maximum=300)
    if result is None:
        return None
    for prefix in ("https://doi.org/", "http://doi.org/", "https://dx.doi.org/", "http://dx.doi.org/", "doi:"):
        if result.lower().startswith(prefix):
            result = result[len(prefix):]
            break
    if not _DOI.fullmatch(result):
        _fail("doi_invalid")
    return result.lower()


def _publication_date(value: Any) -> tuple[str | None, int | None]:
    if value is None:
        return None, None
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{4}(?:-[0-9]{2}(?:-[0-9]{2})?)?", value):
        _fail("publication_date_invalid")
    parts = [int(item) for item in value.split("-")]
    try:
        date(*((parts + [1, 1])[:3] if len(parts) == 1 else parts + [1] if len(parts) == 2 else parts))
    except ValueError:
        _fail("publication_date_invalid")
    return value, parts[0]


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, child in pairs:
        if key in result:
            _fail("duplicate_json_field")
        result[key] = child
    return result


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or len(body) > MAX_RESEARCH_SOURCE_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object,
                           parse_constant=lambda _value: _fail("non_finite_json"))
    except ResearchSourceRetrievalError:
        raise
    except (UnicodeDecodeError, ValueError, RecursionError):
        raise ResearchSourceRetrievalError("invalid_json") from None
    count = 0

    def inspect(child: Any, depth: int = 0) -> None:
        nonlocal count
        count += 1
        if count > MAX_RESEARCH_SOURCE_TREE_NODES or depth > MAX_RESEARCH_SOURCE_TREE_DEPTH:
            _fail("response_tree_bound")
        if isinstance(child, float) and not math.isfinite(child):
            _fail("non_finite_json")
        if isinstance(child, dict):
            for key, item in child.items():
                inspect(key, depth + 1)
                inspect(item, depth + 1)
        elif isinstance(child, list):
            for item in child:
                inspect(item, depth + 1)

    inspect(value)
    if not isinstance(value, dict):
        _fail("response_object_required")
    return value


@dataclass(frozen=True, slots=True)
class ResearchSourceHttpResponse:
    """Caller-owned transport result; final URL is required to verify no redirect occurred."""

    status: int
    body: bytes = field(repr=False)
    final_url: str
    content_type: str = "application/json"


ResearchSourceFetcher = Callable[..., ResearchSourceHttpResponse]


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _pace(source_id: str) -> None:
    with _PACE_LOCK:
        now = time.monotonic()
        delay = max(0.0, _NEXT_REQUEST.get(source_id, now) - now)
        if delay:
            time.sleep(delay)
        _NEXT_REQUEST[source_id] = time.monotonic() + _MIN_REQUEST_INTERVAL[source_id]


def _builtin_fetch(url: str, *, timeout_seconds: float, max_response_bytes: int) -> ResearchSourceHttpResponse:
    request = Request(url, headers={"Accept": "application/json", "User-Agent": "aurora-agent-reviewed-research/0.1"}, method="GET")
    started = time.monotonic()
    try:
        with build_opener(_NoRedirect()).open(request, timeout=timeout_seconds) as response:
            length = response.headers.get("Content-Length")
            if length is not None and (not length.isdigit() or int(length) > max_response_bytes):
                _fail("response_byte_bound")
            chunks = []
            total = 0
            reader = getattr(response, "read1", response.read)
            while True:
                if time.monotonic() - started >= timeout_seconds:
                    _fail("timeout")
                chunk = reader(min(65_536, max_response_bytes + 1 - total))
                if not chunk:
                    break
                chunks.append(chunk)
                total += len(chunk)
                if total > max_response_bytes:
                    _fail("response_byte_bound")
            return ResearchSourceHttpResponse(response.status, b"".join(chunks), response.geturl(), response.headers.get("Content-Type", ""))
    except ResearchSourceRetrievalError:
        raise
    except HTTPError as error:
        failure = "redirect_refused" if 300 <= error.code < 400 else "rate_limited" if error.code == 429 else "http_error"
        raise ResearchSourceRetrievalError(failure, status_code=error.code) from None
    except TimeoutError:
        raise ResearchSourceRetrievalError("timeout") from None
    except (URLError, OSError):
        raise ResearchSourceRetrievalError("transport_error") from None


@dataclass(frozen=True, slots=True)
class ResearchSourceQueryPlan:
    """Pure query plan. Query text stays caller-owned; public projections carry its digest."""

    source_id: str
    _query: str = field(repr=False)
    limit: int = 5

    def __post_init__(self) -> None:
        if not isinstance(self.source_id, str) or self.source_id not in _ENDPOINTS:
            _fail("source_not_reviewed")
        object.__setattr__(self, "_query", _query(self._query))
        _integer(self.limit, 1, MAX_RESEARCH_SOURCE_LIMIT, "limit_bound")

    @property
    def request_url(self) -> str:
        if self.source_id == "openalex":
            parameters = {"search": self._query, "page": "1", "per_page": str(self.limit), "select": "id,doi,display_name,publication_date,publication_year,type"}
        elif self.source_id == "crossref":
            parameters = {"query.bibliographic": self._query, "rows": str(self.limit), "offset": "0", "select": "DOI,title,issued,type"}
        else:
            parameters = {"q": f'"{self._query}"', "page": "1", "size": str(self.limit)}
        return _ENDPOINTS[self.source_id] + "?" + urlencode(parameters)

    @property
    def query_digest(self) -> str:
        return content_digest({"query": self._query})

    def to_dict(self) -> dict[str, Any]:
        descriptor = {"schema": RESEARCH_SOURCE_PLAN_SCHEMA, "source_id": self.source_id,
                      "endpoint": _ENDPOINTS[self.source_id], "query_digest": self.query_digest,
                      "limit": self.limit, "page": 1, "maximum_requests": 1,
                      "request_digest": content_digest({"method": "GET", "url": self.request_url}),
                      "timeout_seconds": RESEARCH_SOURCE_TIMEOUT_SECONDS,
                      "maximum_response_bytes": MAX_RESEARCH_SOURCE_RESPONSE_BYTES,
                      "execution": "review_only;explicit_dispatch_approval_required", "credentials": "not_accepted"}
        return {**descriptor, "plan_digest": content_digest(descriptor)}


def prepare_research_source_query(source_id: str, *, query: str = "glioma", limit: int = 5) -> ResearchSourceQueryPlan:
    return ResearchSourceQueryPlan(source_id, query, limit)


def _record(source_id: str, record_id: str, *, doi: str | None, title: Any, published: Any,
            kind: Any, year: Any = None, date_source: str | None = None) -> dict[str, Any]:
    title = _text(title, "title_invalid")
    publication_date, date_year = _publication_date(published)
    if year is not None:
        year = _integer(year, 1, 9_999, "publication_year_invalid")
        if date_year is not None and year != date_year:
            _fail("publication_year_mismatch")
    else:
        year = date_year
    kind = _text(kind, "record_type_invalid", maximum=128)
    citation = "https://doi.org/" + quote(doi, safe="/():;._-") if doi else record_id if source_id == "openalex" else "https://zenodo.org/records/" + record_id
    result = {"record_id": record_id, "doi": doi, "title": title,
              "publication_date": publication_date, "publication_year": year,
              "publication_date_source": date_source if publication_date is not None else None,
              "record_type": kind, "citation_url": citation}
    result["missing_fields"] = [name for name in ("doi", "title", "publication_date", "publication_year", "record_type") if result[name] is None]
    return result


def _reported_total(value: Any) -> tuple[int | None, str]:
    if value is None:
        return None, "unknown"
    if isinstance(value, dict):
        if set(value) != {"value", "relation"} or value["relation"] not in {"eq", "gte"}:
            _fail("total_count_shape")
        return _integer(value["value"], 0, _MAX_SAFE_INTEGER, "total_count_invalid"), "exact" if value["relation"] == "eq" else "lower_bound"
    return _integer(value, 0, _MAX_SAFE_INTEGER, "total_count_invalid"), "exact"


def _normalise(plan: ResearchSourceQueryPlan, raw: dict[str, Any]) -> tuple[list[dict[str, Any]], int | None, str]:
    source = plan.source_id
    if source == "openalex":
        meta = raw.get("meta")
        rows = raw.get("results")
        if not isinstance(meta, dict):
            _fail("openalex_meta_required")
        for name, expected in (("page", 1), ("per_page", plan.limit)):
            if name in meta and (type(meta[name]) is not int or meta[name] != expected):
                _fail("openalex_page_mismatch")
        total, relation = _reported_total(meta.get("count"))
    elif source == "crossref":
        message = raw.get("message")
        if raw.get("status") != "ok" or raw.get("message-type") != "work-list" or not isinstance(message, dict):
            _fail("crossref_work_list_required")
        rows = message.get("items")
        total, relation = _reported_total(message.get("total-results"))
    else:
        hits = raw.get("hits")
        if not isinstance(hits, dict):
            _fail("zenodo_hits_required")
        rows = hits.get("hits")
        total, relation = _reported_total(hits.get("total"))
    if not isinstance(rows, list) or len(rows) > plan.limit or any(not isinstance(row, dict) for row in rows):
        _fail("record_collection_bound")
    if total is not None and relation == "exact" and total < len(rows):
        _fail("total_count_below_returned_count")
    records = []
    identities = set()
    for row in rows:
        if source == "openalex":
            identity = row.get("id")
            if not isinstance(identity, str) or not _OPENALEX_ID.fullmatch(identity):
                _fail("openalex_identity_invalid")
            record = _record(source, identity, doi=_doi(row.get("doi")), title=row.get("display_name", row.get("title")),
                             published=row.get("publication_date"), year=row.get("publication_year"), kind=row.get("type"), date_source="publication_date")
        elif source == "crossref":
            doi = _doi(row.get("DOI"))
            if doi is None:
                _fail("crossref_doi_required")
            titles = row.get("title")
            if titles is not None and (not isinstance(titles, list) or len(titles) > 16 or any(not isinstance(title, str) for title in titles)):
                _fail("crossref_title_shape")
            issued = row.get("issued")
            published = None
            if issued is not None:
                parts = issued.get("date-parts") if isinstance(issued, dict) else None
                if not isinstance(parts, list) or len(parts) != 1 or not isinstance(parts[0], list) or len(parts[0]) > 3:
                    _fail("crossref_date_shape")
                # Crossref emits [[null]] when an issued date is unavailable. That is missing
                # metadata, never year zero, today's date, or grounds to discard its DOI.
                if parts[0] not in ([], [None]):
                    numbers = [_integer(part, 1, 9_999 if index == 0 else 31, "crossref_date_invalid") for index, part in enumerate(parts[0])]
                    published = "-".join(f"{part:04d}" if index == 0 else f"{part:02d}" for index, part in enumerate(numbers))
            identity = "doi:" + doi
            record = _record(source, identity, doi=doi, title=titles[0] if titles else None, published=published, kind=row.get("type"), date_source="issued")
        else:
            identity = row.get("id")
            if isinstance(identity, str) and re.fullmatch(r"[1-9][0-9]{0,15}", identity):
                identity = int(identity)
            identity = str(_integer(identity, 1, _MAX_SAFE_INTEGER, "zenodo_identity_invalid"))
            metadata = row.get("metadata")
            if not isinstance(metadata, dict):
                _fail("zenodo_metadata_required")
            doi_candidates = [_doi(row.get("doi")), _doi(metadata.get("doi"))]
            pids = row.get("pids")
            if isinstance(pids, dict) and isinstance(pids.get("doi"), dict):
                doi_candidates.append(_doi(pids["doi"].get("identifier")))
            known_dois = [value for value in doi_candidates if value is not None]
            if len(set(known_dois)) > 1:
                _fail("zenodo_doi_mismatch")
            doi = known_dois[0] if known_dois else None
            resource_type = metadata.get("resource_type")
            kind = resource_type.get("type", resource_type.get("id")) if isinstance(resource_type, dict) else resource_type
            record = _record(source, identity, doi=doi, title=metadata.get("title"), published=metadata.get("publication_date"), kind=kind, date_source="publication_date")
        if identity in identities:
            _fail("duplicate_record_identity")
        identities.add(identity)
        records.append(record)
    return records, total, relation


def execute_research_source_query(
    plan: ResearchSourceQueryPlan, *, approve_source_dispatch: bool = False,
    fetcher: ResearchSourceFetcher | None = None, retrieved_at: str | None = None,
) -> WorkflowOperationResult:
    if approve_source_dispatch is not True:
        _fail("explicit_approval_required")
    if type(plan) is not ResearchSourceQueryPlan:
        _fail("exact_reviewed_plan_required")
    if fetcher is not None and not callable(fetcher):
        _fail("fetcher_not_callable")
    timestamp = _timestamp(retrieved_at)
    request_url = plan.request_url
    descriptor = plan.to_dict()
    try:
        if fetcher is None:
            # Crossref's public list pool also limits concurrent requests, so spacing starts
            # alone is insufficient when an earlier response takes longer than the interval.
            with _SOURCE_TRANSPORT_LOCKS[plan.source_id]:
                _pace(plan.source_id)
                response = _builtin_fetch(request_url, timeout_seconds=RESEARCH_SOURCE_TIMEOUT_SECONDS,
                                          max_response_bytes=MAX_RESEARCH_SOURCE_RESPONSE_BYTES)
        else:
            response = fetcher(request_url, timeout_seconds=RESEARCH_SOURCE_TIMEOUT_SECONDS,
                               max_response_bytes=MAX_RESEARCH_SOURCE_RESPONSE_BYTES)
    except ResearchSourceRetrievalError:
        raise
    except Exception:
        raise ResearchSourceRetrievalError("caller_transport_error") from None
    if type(response) is not ResearchSourceHttpResponse:
        _fail("typed_transport_response_required")
    if type(response.status) is not int or response.status != 200:
        status = response.status if type(response.status) is int and 100 <= response.status <= 599 else None
        failure = "redirect_refused" if status is not None and 300 <= status < 400 else "rate_limited" if status == 429 else "http_error"
        raise ResearchSourceRetrievalError(failure, status_code=status)
    if response.final_url != request_url:
        _fail("redirect_refused")
    if not isinstance(response.content_type, str):
        _fail("json_content_type_required")
    media_type = response.content_type.split(";", 1)[0].strip().lower()
    if media_type != "application/json" and not (media_type.startswith("application/") and media_type.endswith("+json")):
        _fail("json_content_type_required")
    raw = _parse(response.body)
    records, total, relation = _normalise(plan, raw)
    returned = len(records)
    omitted = total - returned if relation == "exact" and total is not None else None
    truncated = omitted > 0 if omitted is not None else True if total is not None and total > returned else None
    coverage = "partial" if truncated is True else "complete" if truncated is False else "unknown"
    response_digest = hashlib.sha256(response.body).hexdigest()
    source_digest = content_digest({"source_id": plan.source_id, "request_digest": descriptor["request_digest"], "response_digest": response_digest,
                                    "retrieved_at": timestamp, "records": records, "reported_total_count": total, "total_count_relation": relation})
    bundle = {"schema": RESEARCH_SOURCE_BUNDLE_SCHEMA, "source_id": plan.source_id, "retrieved_at": timestamp,
              "records": records, "returned_count": returned, "reported_total_count": total, "total_count_relation": relation,
              "omitted_count": omitted, "truncated": truncated, "coverage": coverage,
              "query_digest": plan.query_digest, "source_uri": _ENDPOINTS[plan.source_id], "response_digest": response_digest,
              "source_digest": source_digest, "limitations": list(_LIMITATIONS)}
    bundle["bundle_digest"] = content_digest(bundle)
    receipt = {"schema": RESEARCH_SOURCE_RECEIPT_SCHEMA, "source_id": plan.source_id, "plan_digest": descriptor["plan_digest"],
               "request_digest": descriptor["request_digest"], "query_digest": plan.query_digest, "source_digest": source_digest,
               "response_digest": response_digest, "bundle_digest": bundle["bundle_digest"], "retrieved_at": timestamp,
               "request_count": 1, "response_bytes": len(response.body), "returned_count": returned,
               "reported_total_count": total, "total_count_relation": relation, "omitted_count": omitted,
               "truncated": truncated, "coverage": coverage, "network": "builtin_https" if fetcher is None else "caller_transport",
               "effect": "read_only", "retention": "counts_and_digests_only;source_records_transient", "credentials": "not_accepted",
               "limitations": list(_LIMITATIONS)}
    receipt["receipt_digest"] = content_digest(receipt)
    return WorkflowOperationResult(bundle, receipt)


def _operation(source_id: str) -> WorkflowOperation:
    input_schema = {"type": "object", "properties": {
        "query": {"type": "string", "minLength": 1, "maxLength": 200, "pattern": r"^[\w .,'-]+$", "default": "glioma"},
        "limit": {"type": "integer", "minimum": 1, "maximum": MAX_RESEARCH_SOURCE_LIMIT, "default": 5},
    }, "additionalProperties": False}
    row_properties = {name: {"type": ["string", "null"]} for name in ("doi", "title", "publication_date", "publication_date_source", "record_type")}
    row_properties.update({"record_id": {"type": "string"}, "publication_year": {"type": ["integer", "null"]},
                           "citation_url": {"type": "string"}, "missing_fields": {"type": "array", "items": {"type": "string"}}})
    output_schema = {"type": "object", "properties": {
        "schema": {"const": RESEARCH_SOURCE_BUNDLE_SCHEMA}, "source_id": {"const": source_id},
        "records": {"type": "array", "maxItems": MAX_RESEARCH_SOURCE_LIMIT, "items": {"type": "object", "properties": row_properties, "required": list(row_properties), "additionalProperties": False}},
        "returned_count": {"type": "integer", "minimum": 0, "maximum": MAX_RESEARCH_SOURCE_LIMIT},
        "reported_total_count": {"type": ["integer", "null"]}, "total_count_relation": {"enum": ["exact", "lower_bound", "unknown"]},
        "omitted_count": {"type": ["integer", "null"]}, "truncated": {"type": ["boolean", "null"]},
        "coverage": {"enum": ["complete", "partial", "unknown"]},
        "limitations": {"type": "array", "items": {"type": "string"}},
        **{name: {"type": "string"} for name in ("retrieved_at", "query_digest", "source_uri", "response_digest", "source_digest", "bundle_digest")},
    }, "additionalProperties": False}
    output_schema["required"] = list(output_schema["properties"])
    descriptions = {
        "openalex": "Search OpenAlex works for a plain research topic such as glioma. Retrieve one public first page of DOI, title, date and record identity metadata only; anonymous basic-query limits apply.",
        "crossref": "Search Crossref bibliographic work metadata for a plain research topic such as glioma. Retrieve one public first page of DOI, title, issued date and record type only.",
        "zenodo": "Search published Zenodo record identity metadata for a literal research phrase such as glioma. Retrieve one public first page with DOI/title/date/type; never retrieve repository files.",
    }
    return WorkflowOperation(_OPERATION_IDS[source_id], descriptions[source_id], input_schema, output_schema, source_id)


def register_research_source_operations(
    catalog: WorkflowOperationCatalog, *, fetchers: Mapping[str, ResearchSourceFetcher] | None = None,
) -> WorkflowOperationCatalog:
    """Register three concrete readers without performing I/O or obtaining credentials."""
    if not isinstance(catalog, WorkflowOperationCatalog):
        _fail("typed_catalog_required")
    if fetchers is not None and not isinstance(fetchers, Mapping):
        _fail("fetchers_mapping_required")
    transports = {} if fetchers is None else dict(fetchers)
    if set(transports) - set(_ENDPOINTS) or any(not callable(value) for value in transports.values()):
        _fail("fetcher_source_or_type_invalid")
    operations = [_operation(source_id) for source_id in _ENDPOINTS]
    existing = {operation.operation_id for operation in catalog.operations()}
    if existing.intersection(_OPERATION_IDS.values()):
        _fail("research_source_operation_already_registered")
    if len(existing) + len(operations) > MAX_WORKFLOW_OPERATIONS or len({operation.integration_id for operation in catalog.operations()} | set(_ENDPOINTS)) > MAX_WORKFLOW_INTEGRATIONS:
        _fail("research_source_catalog_capacity")
    for operation in operations:
        source_id = operation.integration_id
        fetcher = transports.get(source_id)

        def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool, retrieved_at: str | None,
                    _source_id: str = source_id, _fetcher: ResearchSourceFetcher | None = fetcher) -> WorkflowOperationResult:
            plan = prepare_research_source_query(_source_id, query=arguments["query"], limit=arguments["limit"])
            return execute_research_source_query(plan, approve_source_dispatch=approve_source_dispatch,
                                                 fetcher=_fetcher, retrieved_at=retrieved_at)

        catalog.register(operation, execute)
    return catalog


__all__ = [
    "RESEARCH_SOURCE_PLAN_SCHEMA", "RESEARCH_SOURCE_BUNDLE_SCHEMA", "RESEARCH_SOURCE_RECEIPT_SCHEMA",
    "MAX_RESEARCH_SOURCE_LIMIT", "MAX_RESEARCH_SOURCE_RESPONSE_BYTES", "RESEARCH_SOURCE_TIMEOUT_SECONDS",
    "ResearchSourceRetrievalError", "ResearchSourceHttpResponse", "ResearchSourceFetcher", "ResearchSourceQueryPlan",
    "prepare_research_source_query", "execute_research_source_query", "register_research_source_operations",
]
