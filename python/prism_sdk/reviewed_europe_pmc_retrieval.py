"""Reviewed, bounded Europe PMC publication-metadata retrieval.

The adapter uses a fixed six-lane query catalogue and the Europe PMC ``lite`` search response.
Plans are network-free, source calls require literal approval, and only digests/counts enter the
durable receipt projection. Publication metadata and cursor marks remain transient. A partial or
changing search index is reported as such; it is never promoted to a complete literature review.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass, field
from datetime import date, datetime, timezone
import json
import math
import re
from types import MappingProxyType
from typing import Any
from urllib.parse import urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration


REVIEWED_EUROPE_PMC_CONFIG_SCHEMA = "bioprism-reviewed-europe-pmc-config/0.1"
REVIEWED_EUROPE_PMC_PLAN_SCHEMA = "bioprism-reviewed-europe-pmc-plan/0.1"
REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-europe-pmc-source-receipt/0.1"
REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA = "bioprism-reviewed-europe-pmc-bundle/0.1"
REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA = "bioprism-reviewed-europe-pmc-receipt/0.1"
REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA = "bioprism-reviewed-europe-pmc-transient/0.1"
REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-europe-pmc-execution-metadata/0.1"
REVIEWED_EUROPE_PMC_ADAPTER_VERSION = "0.1"
REVIEWED_EUROPE_PMC_HOST = "www.ebi.ac.uk"
REVIEWED_EUROPE_PMC_PATH = "/europepmc/webservices/rest/search"
REVIEWED_EUROPE_PMC_ENDPOINT = "https://www.ebi.ac.uk/europepmc/webservices/rest/search"
REVIEWED_EUROPE_PMC_AUTHORITY = "Europe PMC / EMBL-EBI"
REVIEWED_EUROPE_PMC_LANES = MappingProxyType({
    "glioma": '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma")',
    "cranial_base": '(("skull base" OR "cranial base" OR petroclival OR "cavernous sinus") AND neurosurgery)',
    "craniosynostosis": '(craniosynostosis OR scaphocephaly OR plagiocephaly OR "Apert syndrome" OR "Crouzon syndrome")',
    "encephalocele": '(encephalocele OR meningoencephalocele OR "basal encephalocele" OR "occipital encephalocele")',
    "spina_bifida": '("spina bifida" OR "spinal dysraphism" OR myelomeningocele OR lipomeningocele OR "tethered cord")',
    "chiari_malformation": '("Chiari malformation" OR syringomyelia OR "craniocervical junction" OR "CSF flow")',
})
MAX_REVIEWED_EUROPE_PMC_PAGE_SIZE = 100
MAX_REVIEWED_EUROPE_PMC_PAGES = 4
MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES = 1_000_000
MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES = 24_000_000
MAX_REVIEWED_EUROPE_PMC_BUNDLE_BYTES = 12_000_000
MAX_REVIEWED_EUROPE_PMC_TREE_DEPTH = 40
MAX_REVIEWED_EUROPE_PMC_TREE_NODES = 100_000
BUILTIN_EUROPE_PMC_TRANSPORT_ID = "builtin.europepmc.urllib"
BUILTIN_EUROPE_PMC_TRANSPORT_VERSION = "1"
BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "python_stdlib_urllib",
    "scheme": "https",
    "host": REVIEWED_EUROPE_PMC_HOST,
    "path": REVIEWED_EUROPE_PMC_PATH,
    "method": "GET",
    "format": "json",
    "result_type": "lite",
    "synonym_expansion": False,
    "redirects": "refused",
    "credentials": "not_accepted",
})

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_CONTROL = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")
_DATE = re.compile(r"^[0-9]{4}(?:-[0-9]{2}(?:-[0-9]{2})?)?$")
_PMID = re.compile(r"^[0-9]{1,12}$")
_PMCID = re.compile(r"^PMC[0-9]{1,12}$", re.IGNORECASE)
_DOI = re.compile(r"^10\.[0-9]{4,9}/[^\s]{1,256}$", re.IGNORECASE)
_RETENTION = "metadata_only;publication_values_and_cursor_marks_transient"
_LIMITATIONS = (
    "Europe PMC lite results are bibliographic metadata, not abstracts or scientific conclusions",
    "fixed specialty keyword queries are bounded discovery aids and do not establish exhaustive coverage",
    "a live search index may change between pages; inconsistent hit counts are reported as unknown",
    "partial results require a later reviewed expansion before any completeness claim",
    "source records require independent review for study quality, omissions, freshness, and applicability",
    "caller-injected transports must enforce timeout, redirect, and network policy under their declared identity",
)


class ReviewedEuropePmcRetrievalError(ValueError):
    """A reviewed Europe PMC request or response violated its bounded contract."""


EuropePmcFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedEuropePmcRetrievalError(message)


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError, RecursionError) as error:
        raise ReviewedEuropePmcRetrievalError(f"{name} is not bounded JSON") from error
    if len(encoded) > maximum:
        _fail(f"{name} exceeds its byte bound")
    return encoded


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _text(name: str, value: Any, maximum: int, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = " ".join(value.split())
    if not normalized and optional:
        return None
    try:
        size = len(normalized.encode("utf-8", errors="strict"))
    except UnicodeEncodeError as error:
        raise ReviewedEuropePmcRetrievalError(f"{name} contains invalid Unicode") from error
    if not normalized or "\x00" in normalized or _CONTROL.search(normalized) or size > maximum:
        _fail(f"{name} is outside its text bound")
    return normalized


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", value):
        _fail(f"{name} must be a UTC timestamp with second precision")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedEuropePmcRetrievalError(f"{name} is not a valid UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _normalize_lanes(value: Sequence[str]) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not 1 <= len(value) <= len(REVIEWED_EUROPE_PMC_LANES):
        _fail("Europe PMC lanes must select one or more fixed catalogue entries")
    lanes = tuple(value)
    if any(not isinstance(lane, str) or lane not in REVIEWED_EUROPE_PMC_LANES for lane in lanes) or len(set(lanes)) != len(lanes):
        _fail("Europe PMC lanes contain an unsupported or duplicate entry")
    return lanes


def _query_set_digest(lanes: Sequence[str]) -> str:
    return content_digest({
        "schema": "bioprism-reviewed-europe-pmc-query-set/0.1",
        "queries": [{"lane": lane, "query": REVIEWED_EUROPE_PMC_LANES[lane]} for lane in lanes],
        "format": "json",
        "result_type": "lite",
        "synonym_expansion": False,
    })


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("Europe PMC response contains a duplicate JSON field")
        result[key] = value
    return result


def _reject_constant(_value: str) -> None:
    _fail("Europe PMC response contains a non-finite number")


def _validate_tree(value: Any) -> None:
    stack = [(value, 0)]
    seen: set[int] = set()
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if nodes > MAX_REVIEWED_EUROPE_PMC_TREE_NODES or depth > MAX_REVIEWED_EUROPE_PMC_TREE_DEPTH:
            _fail("Europe PMC response exceeds its JSON tree bound")
        if isinstance(current, Mapping):
            identity = id(current)
            if identity in seen:
                _fail("Europe PMC response contains a cycle")
            seen.add(identity)
            for key in current:
                if isinstance(key, str):
                    try:
                        key.encode("utf-8", errors="strict")
                    except UnicodeEncodeError as error:
                        raise ReviewedEuropePmcRetrievalError("Europe PMC response contains invalid Unicode") from error
            stack.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            identity = id(current)
            if identity in seen:
                _fail("Europe PMC response contains a cycle")
            seen.add(identity)
            stack.extend((child, depth + 1) for child in current)
        elif isinstance(current, float) and not math.isfinite(current):
            _fail("Europe PMC response contains a non-finite number")
        elif isinstance(current, int) and not isinstance(current, bool) and abs(current) > 9_007_199_254_740_991:
            _fail("Europe PMC response contains an unsafe integer")
        elif isinstance(current, str):
            try:
                current.encode("utf-8", errors="strict")
            except UnicodeEncodeError as error:
                raise ReviewedEuropePmcRetrievalError("Europe PMC response contains invalid Unicode") from error


def _parse_body(value: bytes | str | Mapping[str, Any]) -> tuple[dict[str, Any], int]:
    if isinstance(value, Mapping):
        body = _json_bytes(value, "Europe PMC injected response", MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES)
    elif isinstance(value, str):
        body = value.encode("utf-8", errors="strict")
    elif isinstance(value, bytes):
        body = value
    else:
        _fail("Europe PMC transport returned an unsupported response type")
    if len(body) > MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES:
        _fail("Europe PMC response exceeds its byte bound")
    try:
        decoded = json.loads(body.decode("utf-8", errors="strict"), object_pairs_hook=_pairs_no_duplicates, parse_constant=_reject_constant)
    except ReviewedEuropePmcRetrievalError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError, RecursionError) as error:
        raise ReviewedEuropePmcRetrievalError("Europe PMC response is not valid bounded JSON") from error
    _validate_tree(decoded)
    if not isinstance(decoded, dict):
        _fail("Europe PMC response root must be an object")
    return decoded, len(body)


def _record(raw: Any, source_id: str) -> dict[str, Any]:
    if not isinstance(raw, Mapping):
        _fail("Europe PMC result entry is not an object")
    record_id = _text("Europe PMC record id", raw.get("id"), 32)
    source = _text("Europe PMC record source", raw.get("source"), 16)
    if not _IDENTIFIER.fullmatch(record_id or "") or not _IDENTIFIER.fullmatch(source or ""):
        _fail("Europe PMC record identity is invalid")
    pmid = _text("Europe PMC PMID", raw.get("pmid"), 12, optional=True)
    if pmid is not None and not _PMID.fullmatch(pmid):
        _fail("Europe PMC PMID is invalid")
    pmcid = _text("Europe PMC PMCID", raw.get("pmcid"), 16, optional=True)
    if pmcid is not None:
        pmcid = pmcid.upper()
        if not _PMCID.fullmatch(pmcid):
            _fail("Europe PMC PMCID is invalid")
    doi = _text("Europe PMC DOI", raw.get("doi"), 300, optional=True)
    if doi is not None:
        doi = doi.lower()
        if not _DOI.fullmatch(doi):
            _fail("Europe PMC DOI is invalid")
    if pmid is None and pmcid is None and doi is None:
        _fail("Europe PMC result has no portable publication identifier")
    title = _text("Europe PMC title", raw.get("title"), 8_000)
    year = _text("Europe PMC publication year", raw.get("pubYear"), 4, optional=True)
    if year is not None and (len(year) != 4 or not year.isdigit()):
        _fail("Europe PMC publication year is invalid")
    published = _text("Europe PMC first publication date", raw.get("firstPublicationDate"), 10, optional=True)
    if published is not None:
        if not _DATE.fullmatch(published):
            _fail("Europe PMC first publication date is invalid")
        try:
            if len(published) == 4:
                pass
            elif len(published) == 7:
                date(int(published[:4]), int(published[5:7]), 1)
            else:
                date.fromisoformat(published)
        except ValueError as error:
            raise ReviewedEuropePmcRetrievalError("Europe PMC first publication date is invalid") from error
    cited = raw.get("citedByCount")
    if cited is not None:
        cited = _integer("Europe PMC citedByCount", cited, 0, 9_007_199_254_740_991)
    access = raw.get("isOpenAccess")
    is_open_access = True if access == "Y" else False if access == "N" else None
    return {
        "source_id": source_id,
        "europe_pmc_id": record_id,
        "record_source": source,
        "pmid": pmid,
        "pmcid": pmcid,
        "doi": doi,
        "title": title,
        "author_string": _text("Europe PMC authorString", raw.get("authorString"), 2_000, optional=True),
        "journal_title": _text("Europe PMC journalTitle", raw.get("journalTitle"), 1_000, optional=True),
        "publication_year": year,
        "first_publication_date": published,
        "publication_type": _text("Europe PMC pubType", raw.get("pubType"), 2_000, optional=True),
        "is_open_access": is_open_access,
        "cited_by_count": cited,
    }


def _make_url(lane: str, page_size: int, cursor: str) -> str:
    query = REVIEWED_EUROPE_PMC_LANES[lane]
    parameters = [
        ("query", query), ("format", "json"), ("resultType", "lite"),
        ("pageSize", str(page_size)), ("cursorMark", cursor), ("synonym", "N"),
    ]
    url = f"{REVIEWED_EUROPE_PMC_ENDPOINT}?{urlencode(parameters)}"
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.hostname != REVIEWED_EUROPE_PMC_HOST or parsed.path != REVIEWED_EUROPE_PMC_PATH or len(url) > 8_192:
        _fail("Europe PMC request escaped its reviewed endpoint")
    return url


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req: Request, fp: Any, code: int, msg: str, headers: Any, newurl: str) -> None:
        return None


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    try:
        opener = build_opener(_NoRedirect())
        with opener.open(Request(url, headers={"Accept": "application/json"}), timeout=timeout_ms / 1000) as response:
            if response.status != 200:
                _fail("Europe PMC returned an unsuccessful HTTP status")
            body = response.read(MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES + 1)
    except ReviewedEuropePmcRetrievalError:
        raise
    except Exception as error:
        raise ReviewedEuropePmcRetrievalError("Europe PMC request failed") from error
    if len(body) > MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES:
        _fail("Europe PMC response exceeds its byte bound")
    return body


@dataclass(frozen=True, slots=True)
class ReviewedEuropePmcRetrievalConfig:
    lanes: tuple[str, ...] = ("glioma",)
    page_size: int = 10
    max_pages: int = 2
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_EUROPE_PMC_TRANSPORT_ID
    transport_version: str = BUILTIN_EUROPE_PMC_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST
    _query_set_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        lanes = _normalize_lanes(self.lanes)
        object.__setattr__(self, "lanes", lanes)
        object.__setattr__(self, "page_size", _integer("Europe PMC page_size", self.page_size, 1, MAX_REVIEWED_EUROPE_PMC_PAGE_SIZE))
        object.__setattr__(self, "max_pages", _integer("Europe PMC max_pages", self.max_pages, 1, MAX_REVIEWED_EUROPE_PMC_PAGES))
        object.__setattr__(self, "timeout_ms", _integer("Europe PMC timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER.fullmatch(value):
                _fail(f"Europe PMC {name} is invalid")
        _digest("Europe PMC transport_config_digest", self.transport_config_digest)
        query_digest = _query_set_digest(lanes)
        if self._query_set_digest and _digest("Europe PMC query_set_digest", self._query_set_digest) != query_digest:
            _fail("Europe PMC query_set_digest does not match its fixed lanes")
        object.__setattr__(self, "_query_set_digest", query_digest)
        _json_bytes(self.to_dict(), "Europe PMC config", 32_000)

    @property
    def request_limit(self) -> int:
        return len(self.lanes) * self.max_pages

    @property
    def record_limit(self) -> int:
        return self.request_limit * self.page_size

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_EUROPE_PMC_CONFIG_SCHEMA,
            "lanes": list(self.lanes),
            "page_size": self.page_size,
            "max_pages": self.max_pages,
            "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit,
            "record_limit": self.record_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "query_set_digest": self._query_set_digest,
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedEuropePmcRetrievalConfig":
        expected = {"schema", "lanes", "page_size", "max_pages", "timeout_ms", "request_limit", "record_limit", "transport_id", "transport_version", "transport_config_digest", "query_set_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_EUROPE_PMC_CONFIG_SCHEMA:
            _fail("Europe PMC config has an invalid shape")
        config = cls(tuple(raw["lanes"]), raw["page_size"], raw["max_pages"], raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], raw["query_set_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("Europe PMC config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedEuropePmcRetrievalPlan:
    config: ReviewedEuropePmcRetrievalConfig
    config_digest: str
    query_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedEuropePmcRetrievalConfig) -> "ReviewedEuropePmcRetrievalPlan":
        if type(config) is not ReviewedEuropePmcRetrievalConfig:
            _fail("Europe PMC plan requires an exact config")
        unsigned = {"schema": REVIEWED_EUROPE_PMC_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "query_set_digest": config._query_set_digest, "request_limit": config.request_limit, "record_limit": config.record_limit, "scope": "fixed_public_europe_pmc_lite_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(config, config.config_digest, config._query_set_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {"schema": REVIEWED_EUROPE_PMC_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "query_set_digest": self.query_set_digest, "request_limit": self.config.request_limit, "record_limit": self.config.record_limit, "scope": "fixed_public_europe_pmc_lite_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if type(self.config) is not ReviewedEuropePmcRetrievalConfig or (self.config_digest, self.query_set_digest, self.plan_digest) != (expected.config_digest, expected.query_set_digest, expected.plan_digest):
            _fail("Europe PMC plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedEuropePmcRetrievalPlan":
        expected = {"schema", "config", "config_digest", "query_set_digest", "request_limit", "record_limit", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_EUROPE_PMC_PLAN_SCHEMA:
            _fail("Europe PMC plan has an invalid shape")
        plan = cls.create(ReviewedEuropePmcRetrievalConfig.from_dict(raw["config"]))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("Europe PMC plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedEuropePmcRetrievalResult:
    _bundle: Mapping[str, Any] = field(repr=False)
    _receipt: Mapping[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(dict(self._bundle))

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(dict(self._receipt))

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": self.receipt, "retention": "metadata_only;publication_values_excluded"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_publication_metadata"}


class ReviewedEuropePmcRetrievalAdapter:
    """Execute only the fixed Europe PMC lite search plan after explicit review."""

    def __init__(self, config: ReviewedEuropePmcRetrievalConfig, *, fetch: EuropePmcFetcher | None = None) -> None:
        if type(config) is not ReviewedEuropePmcRetrievalConfig:
            _fail("Europe PMC adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("Europe PMC injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_EUROPE_PMC_TRANSPORT_ID:
            _fail("Europe PMC injected transport requires a distinct reviewed identity")
        if fetch is None and config.transport_id != BUILTIN_EUROPE_PMC_TRANSPORT_ID:
            _fail("Europe PMC built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedEuropePmcRetrievalPlan:
        return ReviewedEuropePmcRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedEuropePmcRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedEuropePmcRetrievalResult:
        if type(plan) is not ReviewedEuropePmcRetrievalPlan:
            _fail("Europe PMC execution requires an exact reviewed plan")
        plan.validate()
        if plan.config != self.config:
            _fail("Europe PMC execution config differs from the reviewed plan")
        if approve_source_dispatch is not True:
            _fail("Europe PMC dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "Europe PMC retrieved_at")
        all_publications: list[dict[str, Any]] = []
        sources: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        request_count = 0
        response_bytes = 0

        for lane in self.config.lanes:
            source_id = f"europepmc_{lane}"
            lane_publications: list[dict[str, Any]] = []
            cursor = "*"
            seen_cursors = {cursor}
            page_hit_counts: list[int] = []
            has_more = False
            for page_index in range(self.config.max_pages):
                url = _make_url(lane, self.config.page_size, cursor)
                try:
                    raw_response = _builtin_fetch(url, self.config.timeout_ms) if self._fetch is None else self._fetch(url)
                except ReviewedEuropePmcRetrievalError:
                    raise
                except Exception as error:
                    raise ReviewedEuropePmcRetrievalError("Europe PMC request failed") from error
                request_count += 1
                response, consumed = _parse_body(raw_response)
                response_bytes += consumed
                if response_bytes > MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES:
                    _fail("Europe PMC aggregate response bytes exceed the plan bound")
                hit_count = _integer("Europe PMC hitCount", response.get("hitCount"), 0, 9_007_199_254_740_991)
                page_hit_counts.append(hit_count)
                result_list = response.get("resultList")
                if not isinstance(result_list, Mapping) or not isinstance(result_list.get("result"), list):
                    _fail("Europe PMC response omitted resultList.result")
                page_records = result_list["result"]
                if len(page_records) > self.config.page_size:
                    _fail("Europe PMC response exceeded the reviewed page size")
                known_ids = {(row["record_source"], row["europe_pmc_id"]) for row in lane_publications}
                for raw_record in page_records:
                    publication = _record(raw_record, source_id)
                    identity = (publication["record_source"], publication["europe_pmc_id"])
                    if identity in known_ids:
                        _fail("Europe PMC pagination returned a duplicate publication")
                    known_ids.add(identity)
                    lane_publications.append(publication)
                next_cursor = response.get("nextCursorMark")
                if next_cursor is None:
                    # Some API versions omit the terminal cursor; never follow a response URL.
                    has_more = "nextPageUrl" in response
                    break
                next_cursor = _text("Europe PMC nextCursorMark", next_cursor, 512)
                if next_cursor == cursor:
                    has_more = False
                    break
                if next_cursor in seen_cursors:
                    _fail("Europe PMC pagination cursor repeated")
                if "nextPageUrl" in response:
                    link = response["nextPageUrl"]
                    if not isinstance(link, str) or len(link.encode("utf-8")) > 2_048:
                        _fail("Europe PMC nextPageUrl is malformed")
                    # It is diagnostic only. Requests are rebuilt from the pinned endpoint.
                has_more = True
                if page_index + 1 >= self.config.max_pages:
                    break
                seen_cursors.add(next_cursor)
                cursor = next_cursor

            if len(all_publications) + len(lane_publications) > self.config.record_limit:
                _fail("Europe PMC record count exceeds the reviewed plan")
            stable_hits = len(set(page_hit_counts)) == 1
            reported_hits = page_hit_counts[0] if stable_hits and page_hit_counts else None
            omitted = None if reported_hits is None or reported_hits < len(lane_publications) else reported_hits - len(lane_publications)
            if reported_hits is not None and reported_hits < len(lane_publications):
                reported_hits = None
                omitted = None
            if has_more or (omitted is not None and omitted > 0):
                completeness = "partial"
            elif reported_hits == len(lane_publications):
                completeness = "complete"
            else:
                completeness = "unknown"
            lane_digest = content_digest(lane_publications)
            sources.append({"source_id": source_id, "authority": REVIEWED_EUROPE_PMC_AUTHORITY, "uri": REVIEWED_EUROPE_PMC_ENDPOINT, "retrieved_at": timestamp, "content_sha256": lane_digest, "record_count": len(lane_publications), "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)})
            source_receipts.append({"schema": REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": source_id, "content_digest": lane_digest, "record_count": len(lane_publications), "reported_hit_count": reported_hits, "omitted_record_count": omitted, "completeness": completeness})
            all_publications.extend(lane_publications)

        status_rank = {"complete": 0, "unknown": 1, "partial": 2}
        completeness = max((row["completeness"] for row in source_receipts), key=status_rank.__getitem__)
        source_set_digest = content_digest(sources)
        bundle_unsigned = {"schema": REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": sources, "publications": all_publications, "source_set_digest": source_set_digest, "record_count": len(all_publications), "completeness": completeness, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "Europe PMC bundle", MAX_REVIEWED_EUROPE_PMC_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        known_totals = all(row["reported_hit_count"] is not None for row in source_receipts)
        known_omitted = all(row["omitted_record_count"] is not None for row in source_receipts)
        reported_total = sum(row["reported_hit_count"] for row in source_receipts) if known_totals else None
        omitted_total = sum(row["omitted_record_count"] for row in source_receipts) if known_omitted else None
        if reported_total is not None and reported_total > 9_007_199_254_740_991:
            reported_total = None
        if omitted_total is not None and omitted_total > 9_007_199_254_740_991:
            omitted_total = None
        receipt_unsigned = {"schema": REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "query_set_digest": plan.query_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_set_digest, "source_count": len(sources), "record_count": len(all_publications), "request_count": request_count, "response_bytes": response_bytes, "reported_hit_count": reported_total, "omitted_record_count": omitted_total, "completeness": completeness, "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedEuropePmcRetrievalResult(deepcopy(bundle), deepcopy(receipt))


def create_reviewed_europe_pmc_execution_metadata(plan: ReviewedEuropePmcRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedEuropePmcRetrievalPlan:
        _fail("Europe PMC execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("Europe PMC execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "Europe PMC retrieved_at")
    payload = {"schema": REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedEuropePmcRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    if not isinstance(value, Mapping) or set(value) != {"schema", "bundle", "receipt", "retention"} or value["schema"] != REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA or value["retention"] != "caller_owned_transient_publication_metadata":
        _fail("Europe PMC transient value is malformed")
    bundle, receipt = value["bundle"], value["receipt"]
    if not isinstance(bundle, Mapping) or not isinstance(receipt, Mapping):
        _fail("Europe PMC transient bundle or receipt is malformed")
    receipt_unsigned = {key: child for key, child in receipt.items() if key != "receipt_digest"}
    bundle_unsigned = {key: child for key, child in bundle.items() if key != "bundle_digest"}
    if content_digest(receipt_unsigned) != _digest("Europe PMC receipt_digest", receipt.get("receipt_digest")) or content_digest(bundle_unsigned) != _digest("Europe PMC bundle_digest", bundle.get("bundle_digest")):
        _fail("Europe PMC transient receipt or bundle digest is invalid")
    bundle_fields = {"schema", "generated_at", "sources", "publications", "source_set_digest", "record_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_fields = {"schema", "plan_digest", "config_digest", "query_set_digest", "bundle_digest", "source_set_digest", "source_count", "record_count", "request_count", "response_bytes", "reported_hit_count", "omitted_record_count", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if set(bundle) != bundle_fields or set(receipt) != receipt_fields:
        _fail("Europe PMC transient bundle or receipt has an invalid shape")
    if bundle.get("schema") != REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA:
        _fail("Europe PMC transient schema is invalid")
    if receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("query_set_digest") != plan.query_set_digest or receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("credentials") != "not_accepted" or receipt.get("provider") != "none" or receipt.get("retention") != _RETENTION:
        _fail("Europe PMC transient receipt is outside the reviewed plan")
    if bundle.get("provider") != "none" or bundle.get("credentials") != "not_accepted" or bundle.get("limitations") != list(_LIMITATIONS) or receipt.get("limitations") != list(_LIMITATIONS):
        _fail("Europe PMC transient boundary metadata is invalid")
    try:
        generated_at = _timestamp(bundle.get("generated_at"), "Europe PMC bundle generated_at")
        retrieved_at = _timestamp(receipt.get("retrieved_at"), "Europe PMC receipt retrieved_at")
    except ReviewedEuropePmcRetrievalError:
        raise
    if generated_at != retrieved_at:
        _fail("Europe PMC bundle and receipt timestamps do not match")
    if bundle.get("source_set_digest") != content_digest(bundle.get("sources")) or receipt.get("source_set_digest") != bundle.get("source_set_digest") or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("Europe PMC transient source binding is invalid")
    if not isinstance(bundle.get("publications"), list) or len(bundle["publications"]) > plan.config.record_limit or _integer("Europe PMC bundle record_count", bundle.get("record_count"), 0, plan.config.record_limit) != len(bundle["publications"]) or _integer("Europe PMC receipt record_count", receipt.get("record_count"), 0, plan.config.record_limit) != len(bundle["publications"]):
        _fail("Europe PMC transient publication count is invalid")
    lane = plan.config.lanes[0]
    source_id = f"europepmc_{lane}"
    sources = bundle.get("sources")
    source_receipts = receipt.get("source_receipts")
    if not isinstance(sources, list) or len(sources) != 1 or not isinstance(source_receipts, list) or len(source_receipts) != 1:
        _fail("Europe PMC transient source cardinality is invalid")
    source = sources[0]
    source_receipt = source_receipts[0]
    if not isinstance(source, Mapping) or set(source) != {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"}:
        _fail("Europe PMC source metadata has an invalid shape")
    if not isinstance(source_receipt, Mapping) or set(source_receipt) != {"schema", "lane", "source_id", "content_digest", "record_count", "reported_hit_count", "omitted_record_count", "completeness"}:
        _fail("Europe PMC source receipt has an invalid shape")
    publications = bundle["publications"]
    seen: set[tuple[str, str]] = set()
    for publication in publications:
        if not isinstance(publication, Mapping) or set(publication) != {"source_id", "europe_pmc_id", "record_source", "pmid", "pmcid", "doi", "title", "author_string", "journal_title", "publication_year", "first_publication_date", "publication_type", "is_open_access", "cited_by_count"}:
            _fail("Europe PMC publication metadata has an invalid shape")
        if publication.get("source_id") != source_id:
            _fail("Europe PMC publication is outside its reviewed lane")
        record_id = _text("Europe PMC record id", publication.get("europe_pmc_id"), 32)
        record_source = _text("Europe PMC record source", publication.get("record_source"), 16)
        if not _IDENTIFIER.fullmatch(record_id or "") or not _IDENTIFIER.fullmatch(record_source or ""):
            _fail("Europe PMC publication identity is invalid")
        identity = (record_source or "", record_id or "")
        if identity in seen:
            _fail("Europe PMC transient publication identity is duplicated")
        seen.add(identity)
        pmid = publication.get("pmid")
        pmcid = publication.get("pmcid")
        doi = publication.get("doi")
        if pmid is not None and (not isinstance(pmid, str) or not _PMID.fullmatch(pmid)):
            _fail("Europe PMC transient PMID is invalid")
        if pmcid is not None and (not isinstance(pmcid, str) or not _PMCID.fullmatch(pmcid) or pmcid != pmcid.upper()):
            _fail("Europe PMC transient PMCID is invalid")
        if doi is not None and (not isinstance(doi, str) or not _DOI.fullmatch(doi) or doi != doi.lower()):
            _fail("Europe PMC transient DOI is invalid")
        if pmid is None and pmcid is None and doi is None:
            _fail("Europe PMC transient publication has no portable identifier")
        for name, maximum in (("title", 8_000), ("author_string", 2_000), ("journal_title", 1_000), ("publication_year", 4), ("first_publication_date", 10), ("publication_type", 2_000)):
            item = publication.get(name)
            if name == "title":
                if _text(f"Europe PMC {name}", item, maximum) != item:
                    _fail("Europe PMC transient title is not normalized")
            elif item is not None and _text(f"Europe PMC {name}", item, maximum) != item:
                _fail("Europe PMC transient publication text is not normalized")
        year = publication.get("publication_year")
        if year is not None and (not isinstance(year, str) or not re.fullmatch(r"[0-9]{4}", year)):
            _fail("Europe PMC transient publication year is invalid")
        published = publication.get("first_publication_date")
        if published is not None:
            if not isinstance(published, str) or not _DATE.fullmatch(published):
                _fail("Europe PMC transient publication date is invalid")
            try:
                if len(published) == 7:
                    date(int(published[:4]), int(published[5:7]), 1)
                elif len(published) == 10:
                    date.fromisoformat(published)
            except ValueError as error:
                raise ReviewedEuropePmcRetrievalError("Europe PMC transient publication date is invalid") from error
        if publication.get("is_open_access") is not None and type(publication.get("is_open_access")) is not bool:
            _fail("Europe PMC transient open-access value is invalid")
        cited = publication.get("cited_by_count")
        if cited is not None:
            _integer("Europe PMC transient cited_by_count", cited, 0, 9_007_199_254_740_991)

    publication_digest = content_digest(publications)
    if source.get("source_id") != source_id or source.get("authority") != REVIEWED_EUROPE_PMC_AUTHORITY or source.get("uri") != REVIEWED_EUROPE_PMC_ENDPOINT or source.get("retrieved_at") != retrieved_at or source.get("content_sha256") != publication_digest or _integer("Europe PMC source record_count", source.get("record_count"), 0, plan.config.record_limit) != len(publications) or source.get("provider") != "none" or source.get("credentials") != "not_accepted" or source.get("limitations") != list(_LIMITATIONS):
        _fail("Europe PMC source metadata does not match its publication bundle")
    reported_hits = source_receipt.get("reported_hit_count")
    omitted = source_receipt.get("omitted_record_count")
    if reported_hits is not None:
        reported_hits = _integer("Europe PMC reported_hit_count", reported_hits, len(publications), 9_007_199_254_740_991)
    if omitted is not None:
        omitted = _integer("Europe PMC omitted_record_count", omitted, 0, 9_007_199_254_740_991)
    if omitted is not None and (reported_hits is None or omitted != reported_hits - len(publications)):
        _fail("Europe PMC omitted-record count does not match its source total")
    source_status = source_receipt.get("completeness")
    if source_status not in {"complete", "unknown", "partial"}:
        _fail("Europe PMC source completeness is invalid")
    expected_status = "partial" if omitted is not None and omitted > 0 else "complete" if reported_hits == len(publications) else "unknown"
    if source_status not in {expected_status, "partial"} or (expected_status == "partial" and source_status != "partial"):
        _fail("Europe PMC source completeness does not match its coverage totals")
    if source_receipt.get("schema") != REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA or source_receipt.get("lane") != lane or source_receipt.get("source_id") != source_id or source_receipt.get("content_digest") != publication_digest or _integer("Europe PMC source receipt record_count", source_receipt.get("record_count"), 0, plan.config.record_limit) != len(publications):
        _fail("Europe PMC source receipt does not match its publication bundle")
    if bundle.get("completeness") != source_status or receipt.get("completeness") != source_status or _integer("Europe PMC receipt source_count", receipt.get("source_count"), 1, 1) != 1 or receipt.get("source_receipts") != [dict(source_receipt)] or receipt.get("reported_hit_count") != reported_hits or receipt.get("omitted_record_count") != omitted:
        _fail("Europe PMC receipt totals do not match its source receipt")
    request_count = _integer("Europe PMC request_count", receipt.get("request_count"), 1, plan.config.max_pages)
    response_bytes = _integer("Europe PMC response_bytes", receipt.get("response_bytes"), 1, MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES)
    if request_count > response_bytes:
        _fail("Europe PMC response byte count is inconsistent with its request count")
    if bundle.get("generated_at") != retrieved_at:
        _fail("Europe PMC bundle timestamp does not match its receipt")
    return bundle, receipt


def create_reviewed_europe_pmc_autonomous_evidence_registration(adapter: ReviewedEuropePmcRetrievalAdapter, plan: ReviewedEuropePmcRetrievalPlan, *, lane: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedEuropePmcRetrievalAdapter or type(plan) is not ReviewedEuropePmcRetrievalPlan:
        _fail("Europe PMC registration requires exact adapter and plan values")
    plan.validate()
    if adapter.config != plan.config or lane not in plan.config.lanes or plan.config.lanes != (lane,):
        _fail("Europe PMC registration requires the exact single-lane plan")
    frozen_plan = ReviewedEuropePmcRetrievalPlan.create(plan.config)
    source_id = f"europepmc_{lane}"
    expected_network = "builtin_https" if adapter._fetch is None else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("Europe PMC acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected:
            _fail("Europe PMC acquisition metadata is malformed")
        unsigned = dict(metadata)
        supplied = _digest("Europe PMC metadata_digest", unsigned.pop("metadata_digest"))
        if supplied != content_digest(unsigned) or metadata.get("schema") != REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("Europe PMC acquisition metadata failed review binding")
        timestamp = None if metadata["retrieved_at"] is None else _timestamp(metadata["retrieved_at"], "Europe PMC retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=timestamp).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("Europe PMC projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(
        adapter_id=f"reviewed.europepmc.{lane}", version=REVIEWED_EUROPE_PMC_ADAPTER_VERSION,
        domains=("biomedical", "neuroscience"), capabilities=("scientific_literature_retrieval", "source_provenance"),
        source_kinds=("europepmc_lite_publication_metadata",), acquire=acquire, project=project,
    )


__all__ = [name for name in globals() if name.startswith("REVIEWED_EUROPE_PMC_") or name.startswith("MAX_REVIEWED_EUROPE_PMC_") or name.startswith("BUILTIN_EUROPE_PMC_")] + [
    "ReviewedEuropePmcRetrievalError", "ReviewedEuropePmcRetrievalConfig", "ReviewedEuropePmcRetrievalPlan",
    "ReviewedEuropePmcRetrievalResult", "ReviewedEuropePmcRetrievalAdapter",
    "EuropePmcFetcher",
    "create_reviewed_europe_pmc_execution_metadata", "create_reviewed_europe_pmc_autonomous_evidence_registration",
]
