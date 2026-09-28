"""Reviewed, bounded retrieval of fixed GWAS Catalog v2 disease associations.

Only direct ontology-trait association pages are queried. Catalog values remain transient;
autonomous evidence receives verified digests and explicit source limitations.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass, field
from datetime import datetime, timezone
import json
import math
import re
import threading
import time
from types import MappingProxyType
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qsl, urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration
from .errors import ArgumentError


REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA = "bioprism-reviewed-gwas-catalog-config/0.1"
REVIEWED_GWAS_CATALOG_PLAN_SCHEMA = "bioprism-reviewed-gwas-catalog-plan/0.1"
REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-source-receipt/0.1"
REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA = "bioprism-reviewed-gwas-catalog-bundle/0.1"
REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-receipt/0.1"
REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA = "bioprism-reviewed-gwas-catalog-transient/0.1"
REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-gwas-catalog-execution-metadata/0.1"
REVIEWED_GWAS_CATALOG_ADAPTER_VERSION = "0.1"
REVIEWED_GWAS_CATALOG_HOST = "www.ebi.ac.uk"
REVIEWED_GWAS_CATALOG_ENDPOINT = f"https://{REVIEWED_GWAS_CATALOG_HOST}/gwas/rest/api/v2/associations"
REVIEWED_GWAS_CATALOG_AUTHORITY = "NHGRI-EBI GWAS Catalog"
REVIEWED_GWAS_CATALOG_LANES = MappingProxyType({
    "gbm": ("MONDO_0018177", "glioblastoma"),
    "glioma": ("MONDO_0021042", "glioma"),
})
MAX_REVIEWED_GWAS_CATALOG_LANES = 2
MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE = 50
MAX_REVIEWED_GWAS_CATALOG_PAGES = 5
MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES = 512_000
MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES = 2_000_000
MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES = 1_000_000
MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH = 20
MAX_REVIEWED_GWAS_CATALOG_TREE_NODES = 80_000
REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS = 100
BUILTIN_GWAS_CATALOG_TRANSPORT_ID = "builtin.gwas-catalog.urllib"
BUILTIN_GWAS_CATALOG_TRANSPORT_VERSION = "1"
_USER_AGENT = "AURORA-Prism-SDK/0.1"
_RETENTION = "bounded_transient_association_metadata;autonomous_evidence_digest_only"
_LIMITATIONS = (
    "GWAS Catalog REST API v2 supplies literature-curated top associations, not complete genome-wide summary statistics",
    "the adapter queries only direct matches to its fixed ontology traits and does not expand child traits",
    "only the first bounded page prefix is retrieved; returned order is source pagination order, not a ranking",
    "p-values and reported traits are copied metadata and do not establish causality, clinical relevance, or treatment benefit",
    "a numeric p-value of zero is retained as source output and flagged as possibly precision-limited, not interpreted as certainty",
    "mapped genes and variant locations are Catalog annotations and are not causal gene or mechanism assignments",
    "the fixed glioblastoma and glioma catalogue is not an exhaustive disease or population search",
    "source records and totals can change as the Catalog is updated",
    "independent review is required for source quality, ancestry, applicability, omissions, and freshness",
    "the adapter does not retrieve patient-level data, full summary statistics, or individual-level genotypes",
    "caller-injected transports own redirect, network, and credential policy under their declared identity",
)
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_ACCESSION_RE = re.compile(r"^GCST[0-9]{5,14}$")
_PUBMED_RE = re.compile(r"^[0-9]{1,12}$")
_GLOBAL_RATE_LOCK = threading.Lock()
_NEXT_REQUEST_AT = 0.0

BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "urllib.request",
    "scheme": "https",
    "host": REVIEWED_GWAS_CATALOG_HOST,
    "path": "/gwas/rest/api/v2/associations",
    "method": "GET",
    "accept": "application/json",
    "user_agent": _USER_AGENT,
    "query": ["efo_id", "show_child_traits=false", "page", "size"],
    "pagination": "validate_and_follow_same_origin_next_link",
    "minimum_request_interval_ms": REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS,
    "redirects": "refused",
    "credentials": "not_accepted",
})


class ReviewedGwasCatalogRetrievalError(ArgumentError):
    """A GWAS Catalog response or reviewed disease plan failed its bounded contract."""


GwasCatalogFetcher = Callable[[str, int], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedGwasCatalogRetrievalError(message)


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its bounded integer range")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        _fail(f"{name} is invalid")
    return value


def _text(name: str, value: Any, maximum: int, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = " ".join(value.split())
    try:
        encoded = normalized.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ReviewedGwasCatalogRetrievalError(f"{name} contains invalid Unicode") from error
    if (not normalized and not allow_empty) or len(encoded) > maximum or any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        _fail(f"{name} is empty or exceeds its text bound")
    return normalized


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(f"{name} must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedGwasCatalogRetrievalError(f"{name} is not a real UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ReviewedGwasCatalogRetrievalError(f"{name} is not bounded JSON") from error
    if len(encoded) > maximum:
        _fail(f"{name} exceeds its byte bound")
    return encoded


def _validate_tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_GWAS_CATALOG_TREE_NODES]
    budget[0] -= 1
    if budget[0] < 0 or depth > MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH:
        _fail("GWAS Catalog response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("GWAS Catalog response contains a non-text key")
            try:
                key.encode("utf-8")
            except UnicodeEncodeError as error:
                raise ReviewedGwasCatalogRetrievalError("GWAS Catalog response contains invalid Unicode") from error
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, str):
        try:
            value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedGwasCatalogRetrievalError("GWAS Catalog response contains invalid Unicode") from error
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("GWAS Catalog response contains a non-finite number")
    elif value is not None and not isinstance(value, (str, int, float, bool)):
        _fail("GWAS Catalog response contains an unsupported value")


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for key, value in pairs:
        if key in output:
            _fail("GWAS Catalog response contains duplicate JSON fields")
        output[key] = value
    return output


def _parse_response(value: Any) -> tuple[Mapping[str, Any], int]:
    if isinstance(value, Mapping):
        encoded = _json_bytes(value, "GWAS Catalog response", MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES)
        parsed: Any = value
    elif isinstance(value, str):
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedGwasCatalogRetrievalError("GWAS Catalog response is not valid UTF-8") from error
        if len(encoded) > MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES:
            _fail("GWAS Catalog response exceeds its byte bound")
        try:
            parsed = json.loads(encoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _value: _fail("GWAS Catalog response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedGwasCatalogRetrievalError("GWAS Catalog response is not valid JSON") from error
    elif isinstance(value, bytes):
        encoded = value
        if len(encoded) > MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES:
            _fail("GWAS Catalog response exceeds its byte bound")
        try:
            parsed = json.loads(encoded.decode("utf-8"), object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _value: _fail("GWAS Catalog response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedGwasCatalogRetrievalError("GWAS Catalog response is not valid JSON") from error
    else:
        _fail("GWAS Catalog response is not JSON")
    if not isinstance(parsed, Mapping):
        _fail("GWAS Catalog response root is not an object")
    _validate_tree(parsed)
    return parsed, len(encoded)


def _query_url(disease_id: str, page: int, page_size: int) -> str:
    return f"{REVIEWED_GWAS_CATALOG_ENDPOINT}?{urlencode((('efo_id', disease_id), ('show_child_traits', 'false'), ('page', str(page)), ('size', str(page_size))))}"


def _validate_page_url(value: Any, disease_id: str, page: int, page_size: int) -> str:
    if not isinstance(value, str) or len(value.encode("utf-8", "strict")) > 2_048:
        _fail("GWAS Catalog pagination link is invalid")
    try:
        parsed = urlsplit(value)
        pairs = parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True)
    except ValueError as error:
        raise ReviewedGwasCatalogRetrievalError("GWAS Catalog pagination link is malformed") from error
    expected = {"efo_id": disease_id, "show_child_traits": "false", "page": str(page), "size": str(page_size)}
    if parsed.scheme != "https" or parsed.hostname != REVIEWED_GWAS_CATALOG_HOST or parsed.port is not None or parsed.username is not None or parsed.password is not None or parsed.path != "/gwas/rest/api/v2/associations" or parsed.fragment or len(pairs) != len(expected) or len({key for key, _ in pairs}) != len(expected) or dict(pairs) != expected:
        _fail("GWAS Catalog pagination link left its reviewed endpoint or query")
    return value


def _pace_requests() -> None:
    global _NEXT_REQUEST_AT
    with _GLOBAL_RATE_LOCK:
        now = time.monotonic()
        start = max(now, _NEXT_REQUEST_AT)
        _NEXT_REQUEST_AT = start + REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS / 1000
    delay = start - now
    if delay > 0:
        time.sleep(delay)


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    request = Request(url, headers={"Accept": "application/json", "User-Agent": _USER_AGENT}, method="GET")
    try:
        with build_opener(_NoRedirect).open(request, timeout=timeout_ms / 1000) as response:
            if response.geturl() != url or response.status != 200:
                _fail("GWAS Catalog endpoint returned an unexpected response")
            result = response.read(MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError) as error:
        raise ReviewedGwasCatalogRetrievalError("GWAS Catalog association request failed") from error
    if len(result) > MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES:
        _fail("GWAS Catalog response exceeds its byte bound")
    return result


def _page_rows(raw: Mapping[str, Any], lane: str, page_number: int, page_size: int) -> tuple[list[Mapping[str, Any]], int, int]:
    if set(raw) not in ({"_links", "page"}, {"_embedded", "_links", "page"}):
        _fail("GWAS Catalog page has an unexpected object shape")
    disease_id = REVIEWED_GWAS_CATALOG_LANES[lane][0]
    page = raw.get("page")
    if not isinstance(page, Mapping) or set(page) != {"size", "totalElements", "totalPages", "number"}:
        _fail("GWAS Catalog page metadata is malformed")
    size = _integer("GWAS Catalog page size", page.get("size"), 1, MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE)
    total = _integer("GWAS Catalog total elements", page.get("totalElements"), 0, 50_000_000)
    total_pages = _integer("GWAS Catalog total pages", page.get("totalPages"), 0, 50_000_000)
    number = _integer("GWAS Catalog page number", page.get("number"), 0, 50_000_000)
    expected_pages = math.ceil(total / size) if total else 0
    if size != page_size or total_pages != expected_pages or number != page_number or (total == 0 and page_number != 0) or page_number >= max(total_pages, 1):
        _fail("GWAS Catalog page metadata differs from the requested page")

    embedded = raw.get("_embedded")
    if total == 0:
        if embedded not in (None, {}) or page_number != 0:
            _fail("empty GWAS Catalog result has unexpected association rows")
        rows: list[Mapping[str, Any]] = []
    else:
        if not isinstance(embedded, Mapping) or set(embedded) != {"associations"} or not isinstance(embedded.get("associations"), list):
            _fail("GWAS Catalog association collection is malformed")
        rows = embedded["associations"]
        if any(not isinstance(row, Mapping) for row in rows):
            _fail("GWAS Catalog association row is malformed")
        expected_count = min(size, total - page_number * size)
        if len(rows) != expected_count:
            _fail("GWAS Catalog association page is incomplete")

    links = raw.get("_links")
    if not isinstance(links, Mapping):
        _fail("GWAS Catalog pagination links are malformed")
    expected_link_keys = {"self"}
    if total_pages:
        expected_link_keys.update({"first", "last"})
        if page_number + 1 < total_pages:
            expected_link_keys.add("next")
    if set(links) != expected_link_keys:
        _fail("GWAS Catalog pagination links do not match page coverage")
    for name, target_page in (("self", page_number), ("first", 0), ("last", max(total_pages - 1, 0)), ("next", page_number + 1)):
        if name in expected_link_keys:
            link = links[name]
            if not isinstance(link, Mapping) or set(link) != {"href"}:
                _fail("GWAS Catalog pagination link is malformed")
            _validate_page_url(link.get("href"), disease_id, target_page, page_size)
    return rows, total, total_pages


def _project_row(raw: Mapping[str, Any], lane: str) -> dict[str, Any]:
    association_id = _integer("GWAS Catalog association_id", raw.get("association_id"), 1, 2**53 - 1)
    accession = _text("GWAS Catalog accession_id", raw.get("accession_id"), 32)
    if not _ACCESSION_RE.fullmatch(accession):
        _fail("GWAS Catalog study accession is invalid")
    raw_traits = raw.get("efo_traits")
    if not isinstance(raw_traits, list) or not raw_traits or len(raw_traits) > 32:
        _fail("GWAS Catalog EFO trait list is malformed")
    traits: list[dict[str, str]] = []
    seen_traits: set[str] = set()
    for item in raw_traits:
        if not isinstance(item, Mapping) or set(item) != {"efo_id", "efo_trait"}:
            _fail("GWAS Catalog EFO trait entry is malformed")
        efo_id = _text("GWAS Catalog efo_id", item.get("efo_id"), 64)
        efo_trait = _text("GWAS Catalog efo_trait", item.get("efo_trait"), 256)
        if efo_id in seen_traits:
            _fail("GWAS Catalog EFO trait list contains duplicates")
        seen_traits.add(efo_id)
        traits.append({"efo_id": efo_id, "efo_trait": efo_trait})
    disease_id = REVIEWED_GWAS_CATALOG_LANES[lane][0]
    if disease_id not in seen_traits:
        _fail("GWAS Catalog association does not match the exact reviewed ontology trait")

    reported_raw = raw.get("reported_trait")
    if not isinstance(reported_raw, list) or len(reported_raw) > 64:
        _fail("GWAS Catalog reported_trait list is malformed")
    reported_traits = [_text("GWAS Catalog reported trait", item, 512) for item in reported_raw]
    genes_raw = raw.get("mapped_genes")
    locations_raw = raw.get("locations")
    if not isinstance(genes_raw, list) or len(genes_raw) > 256 or not isinstance(locations_raw, list) or len(locations_raw) > 256:
        _fail("GWAS Catalog mapped gene or location list is malformed")
    genes = [_text("GWAS Catalog mapped gene", item, 128) for item in genes_raw]
    locations = [_text("GWAS Catalog location", item, 128) for item in locations_raw]
    if len(set(genes)) != len(genes) or len(set(locations)) != len(locations):
        _fail("GWAS Catalog association contains duplicate gene or location rows")

    pubmed_value = raw.get("pubmed_id")
    if isinstance(pubmed_value, int) and not isinstance(pubmed_value, bool):
        pubmed_id = str(_integer("GWAS Catalog pubmed_id", pubmed_value, 1, 999_999_999_999))
    elif isinstance(pubmed_value, str):
        pubmed_id = _text("GWAS Catalog pubmed_id", pubmed_value, 12)
    elif pubmed_value is None:
        pubmed_id = None
    else:
        _fail("GWAS Catalog pubmed_id has an unsupported type")
    if pubmed_id is not None and not _PUBMED_RE.fullmatch(pubmed_id):
        _fail("GWAS Catalog pubmed_id is invalid")

    p_value_raw = raw.get("p_value")
    if p_value_raw is None:
        p_value = None
        p_value_state = "not_reported"
    elif isinstance(p_value_raw, (int, float)) and not isinstance(p_value_raw, bool) and math.isfinite(p_value_raw) and 0 <= p_value_raw <= 1:
        p_value = p_value_raw
        if p_value == 0 and math.copysign(1.0, p_value) < 0:
            _fail("GWAS Catalog p_value uses a negative zero")
        p_value_state = "reported_zero_or_underflow" if p_value == 0 else "reported"
    else:
        _fail("GWAS Catalog p_value is invalid")
    first_author_raw = raw.get("first_author")
    first_author = None if first_author_raw is None else _text("GWAS Catalog first_author", first_author_raw, 256)
    return {
        "association_id": association_id,
        "study_accession": accession,
        "pubmed_id": pubmed_id,
        "first_author": first_author,
        "p_value": p_value,
        "p_value_state": p_value_state,
        "efo_traits": traits,
        "reported_traits": reported_traits,
        "mapped_genes": genes,
        "locations": locations,
    }


def _validate_projected_row(raw: Any, lane: str) -> dict[str, Any]:
    expected = {"association_id", "study_accession", "pubmed_id", "first_author", "p_value", "p_value_state", "efo_traits", "reported_traits", "mapped_genes", "locations"}
    if not isinstance(raw, Mapping) or set(raw) != expected:
        _fail("GWAS Catalog projected association row has an invalid shape")
    normalized = _project_row({
        "association_id": raw.get("association_id"),
        "accession_id": raw.get("study_accession"),
        "pubmed_id": raw.get("pubmed_id"),
        "first_author": raw.get("first_author"),
        "p_value": raw.get("p_value"),
        "efo_traits": raw.get("efo_traits"),
        "reported_trait": raw.get("reported_traits"),
        "mapped_genes": raw.get("mapped_genes"),
        "locations": raw.get("locations"),
    }, lane)
    if raw.get("p_value_state") != normalized["p_value_state"] or canonical_json(dict(raw)) != canonical_json(normalized):
        _fail("GWAS Catalog projected association row is not normalized")
    return normalized


@dataclass(frozen=True, slots=True)
class ReviewedGwasCatalogRetrievalConfig:
    lanes: tuple[str, ...] = ("gbm", "glioma")
    page_size: int = 20
    max_pages: int = 2
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_GWAS_CATALOG_TRANSPORT_ID
    transport_version: str = BUILTIN_GWAS_CATALOG_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST
    _lane_set_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.lanes, Sequence) or isinstance(self.lanes, (str, bytes)):
            _fail("GWAS Catalog lanes must be a bounded list")
        lanes = tuple(self.lanes)
        if not lanes or len(lanes) > MAX_REVIEWED_GWAS_CATALOG_LANES or any(not isinstance(lane, str) or lane not in REVIEWED_GWAS_CATALOG_LANES for lane in lanes) or len(set(lanes)) != len(lanes):
            _fail("GWAS Catalog lanes contain an unsupported or duplicate lane")
        canonical_lanes = tuple(lane for lane in REVIEWED_GWAS_CATALOG_LANES if lane in lanes)
        object.__setattr__(self, "lanes", canonical_lanes)
        object.__setattr__(self, "page_size", _integer("GWAS Catalog page_size", self.page_size, 1, MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE))
        object.__setattr__(self, "max_pages", _integer("GWAS Catalog max_pages", self.max_pages, 1, MAX_REVIEWED_GWAS_CATALOG_PAGES))
        object.__setattr__(self, "timeout_ms", _integer("GWAS Catalog timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER_RE.fullmatch(value):
                _fail(f"GWAS Catalog {name} is invalid")
        _digest("GWAS Catalog transport_config_digest", self.transport_config_digest)
        lane_digest = content_digest(self._lane_payload(canonical_lanes))
        if self._lane_set_digest and _digest("GWAS Catalog lane_set_digest", self._lane_set_digest) != lane_digest:
            _fail("GWAS Catalog lane_set_digest does not match the fixed disease catalogue")
        object.__setattr__(self, "_lane_set_digest", lane_digest)
        _json_bytes(self.to_dict(), "GWAS Catalog config", 32_000)

    @staticmethod
    def _lane_payload(lanes: Sequence[str]) -> dict[str, Any]:
        return {"diseases": [{"lane": lane, "disease_id": REVIEWED_GWAS_CATALOG_LANES[lane][0], "disease_name": REVIEWED_GWAS_CATALOG_LANES[lane][1]} for lane in lanes]}

    @property
    def request_limit(self) -> int:
        return len(self.lanes) * self.max_pages

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA,
            "lanes": list(self.lanes),
            "page_size": self.page_size,
            "max_pages": self.max_pages,
            "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "lane_set_digest": self._lane_set_digest,
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedGwasCatalogRetrievalConfig":
        expected = {"schema", "lanes", "page_size", "max_pages", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "lane_set_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA or not isinstance(raw.get("lanes"), list):
            _fail("GWAS Catalog config has an invalid shape")
        config = cls(tuple(raw["lanes"]), raw["page_size"], raw["max_pages"], raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], raw["lane_set_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("GWAS Catalog config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedGwasCatalogRetrievalPlan:
    config: ReviewedGwasCatalogRetrievalConfig
    config_digest: str
    lane_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedGwasCatalogRetrievalConfig) -> "ReviewedGwasCatalogRetrievalPlan":
        if type(config) is not ReviewedGwasCatalogRetrievalConfig:
            _fail("GWAS Catalog plan requires an exact config")
        unsigned = {
            "schema": REVIEWED_GWAS_CATALOG_PLAN_SCHEMA,
            "config": config.to_dict(),
            "config_digest": config.config_digest,
            "lane_set_digest": config._lane_set_digest,
            "request_limit": config.request_limit,
            "maximum_returned_associations": len(config.lanes) * config.page_size * config.max_pages,
            "minimum_request_interval_ms": REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS,
            "scope": "fixed_glioma_direct_ontology_trait_association_pages",
            "execution": "bounded_https_get_and_validated_same_origin_pagination_after_literal_approval",
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }
        return cls(config, config.config_digest, config._lane_set_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {
            "schema": REVIEWED_GWAS_CATALOG_PLAN_SCHEMA,
            "config": self.config.to_dict(),
            "config_digest": self.config_digest,
            "lane_set_digest": self.lane_set_digest,
            "request_limit": self.config.request_limit,
            "maximum_returned_associations": len(self.config.lanes) * self.config.page_size * self.config.max_pages,
            "minimum_request_interval_ms": REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS,
            "scope": "fixed_glioma_direct_ontology_trait_association_pages",
            "execution": "bounded_https_get_and_validated_same_origin_pagination_after_literal_approval",
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if (self.config_digest, self.lane_set_digest, self.plan_digest) != (expected.config_digest, expected.lane_set_digest, expected.plan_digest):
            _fail("GWAS Catalog plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedGwasCatalogRetrievalPlan":
        expected = {"schema", "config", "config_digest", "lane_set_digest", "request_limit", "maximum_returned_associations", "minimum_request_interval_ms", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_GWAS_CATALOG_PLAN_SCHEMA:
            _fail("GWAS Catalog plan has an invalid shape")
        plan = cls.create(ReviewedGwasCatalogRetrievalConfig.from_dict(raw["config"]))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("GWAS Catalog plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedGwasCatalogRetrievalResult:
    _bundle: Mapping[str, Any] = field(repr=False)
    _receipt: Mapping[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(dict(self._bundle))

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(dict(self._receipt))

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": self.receipt, "retention": "digest_metadata_only"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_association_metadata"}


class ReviewedGwasCatalogRetrievalAdapter:
    """Read fixed, bounded GWAS Catalog association page prefixes after explicit review."""

    def __init__(self, config: ReviewedGwasCatalogRetrievalConfig, *, fetch: GwasCatalogFetcher | None = None) -> None:
        if type(config) is not ReviewedGwasCatalogRetrievalConfig:
            _fail("GWAS Catalog adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("GWAS Catalog injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_GWAS_CATALOG_TRANSPORT_ID:
            _fail("GWAS Catalog injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id != BUILTIN_GWAS_CATALOG_TRANSPORT_ID or config.transport_config_digest != BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST):
            _fail("GWAS Catalog built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedGwasCatalogRetrievalPlan:
        return ReviewedGwasCatalogRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedGwasCatalogRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedGwasCatalogRetrievalResult:
        if type(plan) is not ReviewedGwasCatalogRetrievalPlan:
            _fail("GWAS Catalog execution requires an exact reviewed plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("GWAS Catalog execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("GWAS Catalog dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "GWAS Catalog retrieved_at")
        lanes: list[dict[str, Any]] = []
        sources: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        response_bytes = 0
        request_count = 0
        for lane in self.config.lanes:
            disease_id, disease_name = REVIEWED_GWAS_CATALOG_LANES[lane]
            associations: list[dict[str, Any]] = []
            total: int | None = None
            total_pages: int | None = None
            pages_requested = 0
            seen_ids: set[int] = set()
            page_limit = self.config.max_pages
            for page_number in range(page_limit):
                if total_pages is not None and page_number >= total_pages:
                    break
                url = _query_url(disease_id, page_number, self.config.page_size)
                _pace_requests()
                try:
                    raw = _builtin_fetch(url, self.config.timeout_ms) if self._fetch is None else self._fetch(url, self.config.timeout_ms)
                except ReviewedGwasCatalogRetrievalError:
                    raise
                except Exception as error:
                    raise ReviewedGwasCatalogRetrievalError("GWAS Catalog association request failed") from error
                response, consumed = _parse_response(raw)
                response_bytes += consumed
                request_count += 1
                pages_requested += 1
                if response_bytes > MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES:
                    _fail("GWAS Catalog aggregate response bytes exceed the plan bound")
                raw_rows, page_total, page_total_pages = _page_rows(response, lane, page_number, self.config.page_size)
                if total is not None and (page_total != total or page_total_pages != total_pages):
                    _fail("GWAS Catalog totals changed during paginated retrieval")
                total, total_pages = page_total, page_total_pages
                for raw_row in raw_rows:
                    row = _project_row(raw_row, lane)
                    if row["association_id"] in seen_ids:
                        _fail("GWAS Catalog page prefix contains duplicate association identifiers")
                    seen_ids.add(row["association_id"])
                    associations.append(row)
            if total is None or total_pages is None:
                _fail("GWAS Catalog retrieval produced no validated first page")
            expected_returned = min(total, self.config.page_size * min(total_pages, self.config.max_pages))
            if len(associations) != expected_returned:
                _fail("GWAS Catalog retrieved page prefix is incomplete")
            coverage = "all_source_rows" if total_pages <= self.config.max_pages else "bounded_page_prefix"
            lane_record = {
                "source_id": f"gwas_catalog_{lane}",
                "lane": lane,
                "disease_id": disease_id,
                "disease_name": disease_name,
                "total_associations": total,
                "returned_associations": len(associations),
                "omitted_associations": total - len(associations),
                "page_size": self.config.page_size,
                "max_pages": self.config.max_pages,
                "pages_requested": pages_requested,
                "coverage": coverage,
                "query_semantics": "exact_ontology_trait_direct_matches_only",
                "associations": associations,
            }
            lane_digest = content_digest(lane_record)
            query_digest = content_digest({"endpoint": REVIEWED_GWAS_CATALOG_ENDPOINT, "disease_id": disease_id, "show_child_traits": False, "page_size": self.config.page_size, "max_pages": self.config.max_pages})
            sources.append({"source_id": lane_record["source_id"], "authority": REVIEWED_GWAS_CATALOG_AUTHORITY, "uri": REVIEWED_GWAS_CATALOG_ENDPOINT, "query_digest": query_digest, "retrieved_at": timestamp, "content_sha256": lane_digest, "record_count": len(associations), "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)})
            source_receipts.append({"schema": REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": lane_record["source_id"], "disease_id": disease_id, "content_digest": lane_digest, "total_associations": total, "returned_associations": len(associations), "omitted_associations": total - len(associations), "pages_requested": pages_requested, "total_pages": total_pages, "coverage": coverage})
            lanes.append(lane_record)

        coverage = "all_source_rows" if all(row["coverage"] == "all_source_rows" for row in lanes) else "bounded_page_prefix"
        association_count = sum(row["returned_associations"] for row in lanes)
        source_set_digest = content_digest(sources)
        bundle_unsigned = {"schema": REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": sources, "lanes": lanes, "source_set_digest": source_set_digest, "lane_count": len(lanes), "association_count": association_count, "coverage": coverage, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "GWAS Catalog bundle", MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        receipt_unsigned = {"schema": REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "lane_set_digest": plan.lane_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_set_digest, "source_count": len(sources), "lane_count": len(lanes), "association_count": association_count, "request_count": request_count, "response_bytes": response_bytes, "coverage": coverage, "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedGwasCatalogRetrievalResult(bundle, receipt)


def create_reviewed_gwas_catalog_execution_metadata(plan: ReviewedGwasCatalogRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedGwasCatalogRetrievalPlan:
        _fail("GWAS Catalog execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("GWAS Catalog execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "GWAS Catalog retrieved_at")
    payload = {"schema": REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedGwasCatalogRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    _json_bytes(value, "GWAS Catalog transient value", MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES + 100_000)
    expected_top = {"schema", "bundle", "receipt", "retention"}
    if not isinstance(value, Mapping) or set(value) != expected_top or value.get("schema") != REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA or value.get("retention") != "caller_owned_transient_association_metadata":
        _fail("GWAS Catalog transient value is malformed")
    bundle = value.get("bundle")
    receipt = value.get("receipt")
    bundle_keys = {"schema", "generated_at", "sources", "lanes", "source_set_digest", "lane_count", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_keys = {"schema", "plan_digest", "config_digest", "lane_set_digest", "bundle_digest", "source_set_digest", "source_count", "lane_count", "association_count", "request_count", "response_bytes", "coverage", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if not isinstance(bundle, Mapping) or set(bundle) != bundle_keys or not isinstance(receipt, Mapping) or set(receipt) != receipt_keys:
        _fail("GWAS Catalog transient bundle or receipt has an invalid shape")
    _json_bytes(bundle, "GWAS Catalog transient bundle", MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES)
    bundle_unsigned = {key: item for key, item in bundle.items() if key != "bundle_digest"}
    receipt_unsigned = {key: item for key, item in receipt.items() if key != "receipt_digest"}
    if content_digest(bundle_unsigned) != _digest("GWAS Catalog bundle digest", bundle.get("bundle_digest")) or content_digest(receipt_unsigned) != _digest("GWAS Catalog receipt digest", receipt.get("receipt_digest")):
        _fail("GWAS Catalog transient digests are invalid")
    if bundle.get("schema") != REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA or receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("lane_set_digest") != plan.lane_set_digest:
        _fail("GWAS Catalog transient identity differs from the reviewed plan")
    if receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("provider") != "none" or receipt.get("credentials") != "not_accepted" or receipt.get("retention") != _RETENTION or bundle.get("provider") != "none" or bundle.get("credentials") != "not_accepted":
        _fail("GWAS Catalog transient boundary metadata is invalid")
    if canonical_json(bundle.get("limitations")) != canonical_json(_LIMITATIONS) or canonical_json(receipt.get("limitations")) != canonical_json(_LIMITATIONS):
        _fail("GWAS Catalog transient limitations are invalid")
    retrieved_at = _timestamp(receipt.get("retrieved_at"), "GWAS Catalog retrieved_at")
    if _timestamp(bundle.get("generated_at"), "GWAS Catalog generated_at") != retrieved_at:
        _fail("GWAS Catalog bundle and receipt timestamps do not match")
    lanes = bundle.get("lanes")
    sources = bundle.get("sources")
    source_receipts = receipt.get("source_receipts")
    if not isinstance(lanes, list) or not isinstance(sources, list) or not isinstance(source_receipts, list) or len(lanes) != len(plan.config.lanes) or len(sources) != len(lanes) or len(source_receipts) != len(lanes):
        _fail("GWAS Catalog transient lane coverage is incomplete")
    source_set_digest = _digest("GWAS Catalog source_set_digest", bundle.get("source_set_digest"))
    if content_digest(sources) != source_set_digest or receipt.get("source_set_digest") != source_set_digest or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("GWAS Catalog transient source-set binding is invalid")
    total_associations = 0
    expected_requests = 0
    expected_coverage: list[str] = []
    timestamp = retrieved_at
    for index, lane in enumerate(plan.config.lanes):
        record = lanes[index]
        record_keys = {"source_id", "lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "page_size", "max_pages", "pages_requested", "coverage", "query_semantics", "associations"}
        if not isinstance(record, Mapping) or set(record) != record_keys:
            _fail("GWAS Catalog lane record has an invalid shape")
        disease_id, disease_name = REVIEWED_GWAS_CATALOG_LANES[lane]
        total = _integer("GWAS Catalog total associations", record.get("total_associations"), 0, 50_000_000)
        total_pages = math.ceil(total / plan.config.page_size) if total else 0
        pages_requested = min(total_pages, plan.config.max_pages) if total_pages else 1
        returned = min(total, plan.config.page_size * plan.config.max_pages)
        omitted = total - returned
        lane_coverage = "all_source_rows" if total_pages <= plan.config.max_pages else "bounded_page_prefix"
        rows = record.get("associations")
        if not isinstance(rows, list) or len(rows) != returned or record.get("source_id") != f"gwas_catalog_{lane}" or record.get("lane") != lane or record.get("disease_id") != disease_id or record.get("disease_name") != disease_name or record.get("returned_associations") != returned or record.get("omitted_associations") != omitted or record.get("page_size") != plan.config.page_size or record.get("max_pages") != plan.config.max_pages or record.get("pages_requested") != pages_requested or record.get("coverage") != lane_coverage or record.get("query_semantics") != "exact_ontology_trait_direct_matches_only":
            _fail("GWAS Catalog lane projection differs from its reviewed query")
        seen_ids: set[int] = set()
        for raw_row in rows:
            row = _validate_projected_row(raw_row, lane)
            if row["association_id"] in seen_ids:
                _fail("GWAS Catalog projected lane repeats an association identifier")
            seen_ids.add(row["association_id"])
        lane_digest = content_digest(record)
        query_digest = content_digest({"endpoint": REVIEWED_GWAS_CATALOG_ENDPOINT, "disease_id": disease_id, "show_child_traits": False, "page_size": plan.config.page_size, "max_pages": plan.config.max_pages})
        expected_source = {"source_id": f"gwas_catalog_{lane}", "authority": REVIEWED_GWAS_CATALOG_AUTHORITY, "uri": REVIEWED_GWAS_CATALOG_ENDPOINT, "query_digest": query_digest, "retrieved_at": timestamp, "content_sha256": lane_digest, "record_count": returned, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        source = sources[index]
        if not isinstance(source, Mapping) or canonical_json(dict(source)) != canonical_json(expected_source):
            _fail("GWAS Catalog source metadata does not match its lane record")
        expected_source_receipt = {"schema": REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": f"gwas_catalog_{lane}", "disease_id": disease_id, "content_digest": lane_digest, "total_associations": total, "returned_associations": returned, "omitted_associations": omitted, "pages_requested": pages_requested, "total_pages": total_pages, "coverage": lane_coverage}
        if not isinstance(source_receipts[index], Mapping) or canonical_json(dict(source_receipts[index])) != canonical_json(expected_source_receipt):
            _fail("GWAS Catalog source receipt does not match its lane record")
        total_associations += returned
        expected_requests += pages_requested
        expected_coverage.append(lane_coverage)

    coverage = "all_source_rows" if all(item == "all_source_rows" for item in expected_coverage) else "bounded_page_prefix"
    if bundle.get("coverage") != coverage or receipt.get("coverage") != coverage or _integer("GWAS Catalog bundle lane_count", bundle.get("lane_count"), 1, MAX_REVIEWED_GWAS_CATALOG_LANES) != len(lanes) or _integer("GWAS Catalog receipt lane_count", receipt.get("lane_count"), 1, MAX_REVIEWED_GWAS_CATALOG_LANES) != len(lanes) or _integer("GWAS Catalog source_count", receipt.get("source_count"), 1, MAX_REVIEWED_GWAS_CATALOG_LANES) != len(lanes) or _integer("GWAS Catalog association_count", bundle.get("association_count"), 0, len(plan.config.lanes) * plan.config.page_size * plan.config.max_pages) != total_associations or _integer("GWAS Catalog receipt association_count", receipt.get("association_count"), 0, len(plan.config.lanes) * plan.config.page_size * plan.config.max_pages) != total_associations or _integer("GWAS Catalog request_count", receipt.get("request_count"), 1, plan.config.request_limit) != expected_requests:
        _fail("GWAS Catalog transient counts or coverage are inconsistent")
    _integer("GWAS Catalog response_bytes", receipt.get("response_bytes"), expected_requests, MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES)
    return bundle, receipt


def create_reviewed_gwas_catalog_autonomous_evidence_registration(adapter: ReviewedGwasCatalogRetrievalAdapter, plan: ReviewedGwasCatalogRetrievalPlan, *, lane: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedGwasCatalogRetrievalAdapter or type(plan) is not ReviewedGwasCatalogRetrievalPlan:
        _fail("GWAS Catalog registration requires exact adapter and plan values")
    plan.validate()
    if canonical_json(adapter.config.to_dict()) != canonical_json(plan.config.to_dict()) or lane not in plan.config.lanes or len(plan.config.lanes) != 1:
        _fail("GWAS Catalog registration requires the exact single-lane plan")
    frozen_plan = ReviewedGwasCatalogRetrievalPlan.create(plan.config)
    source_id = f"gwas_catalog_{lane}"
    expected_network = "builtin_https" if adapter.config.transport_id == BUILTIN_GWAS_CATALOG_TRANSPORT_ID else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("GWAS Catalog acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected_keys = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected_keys:
            _fail("GWAS Catalog execution metadata has an invalid shape")
        unsigned = {key: item for key, item in metadata.items() if key != "metadata_digest"}
        if content_digest(unsigned) != _digest("GWAS Catalog metadata_digest", metadata.get("metadata_digest")) or metadata.get("schema") != REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("GWAS Catalog execution metadata failed review binding")
        retrieved_at = None if metadata.get("retrieved_at") is None else _timestamp(metadata.get("retrieved_at"), "GWAS Catalog retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=retrieved_at).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("GWAS Catalog evidence projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(
        adapter_id=f"reviewed.gwas_catalog.{lane}",
        version=REVIEWED_GWAS_CATALOG_ADAPTER_VERSION,
        domains=("biomedical", "neuroscience"),
        capabilities=("human_gwas_association_metadata", "source_provenance"),
        source_kinds=("gwas_catalog_direct_trait_association_metadata",),
        acquire=acquire,
        project=project,
    )


__all__ = [name for name in globals() if name.startswith("REVIEWED_GWAS_CATALOG_") or name.startswith("MAX_REVIEWED_GWAS_CATALOG_") or name.startswith("BUILTIN_GWAS_CATALOG_")] + [
    "GwasCatalogFetcher", "ReviewedGwasCatalogRetrievalError", "ReviewedGwasCatalogRetrievalConfig", "ReviewedGwasCatalogRetrievalPlan", "ReviewedGwasCatalogRetrievalResult", "ReviewedGwasCatalogRetrievalAdapter", "create_reviewed_gwas_catalog_execution_metadata", "create_reviewed_gwas_catalog_autonomous_evidence_registration",
]
