"""Reviewed, fixed-catalogue NCBI Gene summary retrieval (blueprint module 11.04, Python SDK).

This source adapter queries only fixed human GeneIDs through NCBI E-utilities ESummary. It
projects bounded identity and location metadata; source rows remain transient and autonomous
evidence receives only source and bundle digests. It does not interpret gene function or variants.
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
from urllib.parse import urlencode
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration
from .errors import ArgumentError


REVIEWED_NCBI_GENE_CONFIG_SCHEMA = "bioprism-reviewed-ncbi-gene-config/0.1"
REVIEWED_NCBI_GENE_PLAN_SCHEMA = "bioprism-reviewed-ncbi-gene-plan/0.1"
REVIEWED_NCBI_GENE_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-ncbi-gene-source-receipt/0.1"
REVIEWED_NCBI_GENE_BUNDLE_SCHEMA = "bioprism-reviewed-ncbi-gene-bundle/0.1"
REVIEWED_NCBI_GENE_RECEIPT_SCHEMA = "bioprism-reviewed-ncbi-gene-receipt/0.1"
REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA = "bioprism-reviewed-ncbi-gene-transient/0.1"
REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-ncbi-gene-execution-metadata/0.1"
REVIEWED_NCBI_GENE_ADAPTER_VERSION = "0.1"
REVIEWED_NCBI_GENE_HOST = "eutils.ncbi.nlm.nih.gov"
REVIEWED_NCBI_GENE_PATH = "/entrez/eutils/esummary.fcgi"
REVIEWED_NCBI_GENE_ENDPOINT = f"https://{REVIEWED_NCBI_GENE_HOST}{REVIEWED_NCBI_GENE_PATH}"
REVIEWED_NCBI_GENE_AUTHORITY = "NCBI Gene"
REVIEWED_NCBI_GENE_CATALOGUE = MappingProxyType({
    "IDH1": "3417", "IDH2": "3418", "MGMT": "4255", "EGFR": "1956",
    "TERT": "7015", "TP53": "7157", "ATRX": "546", "NF1": "4763",
    "PTEN": "5728", "CDKN2A": "1029", "PDGFRA": "5156", "BRAF": "673",
})
MAX_REVIEWED_NCBI_GENE_SYMBOLS = len(REVIEWED_NCBI_GENE_CATALOGUE)
MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES = 1_000_000
MAX_REVIEWED_NCBI_GENE_BUNDLE_BYTES = 256_000
MAX_REVIEWED_NCBI_GENE_TREE_DEPTH = 32
MAX_REVIEWED_NCBI_GENE_TREE_NODES = 50_000
BUILTIN_NCBI_GENE_TRANSPORT_ID = "builtin.ncbi-gene.urllib"
BUILTIN_NCBI_GENE_TRANSPORT_VERSION = "1"
BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "urllib.request", "scheme": "https", "host": REVIEWED_NCBI_GENE_HOST,
    "path": REVIEWED_NCBI_GENE_PATH, "method": "GET", "database": "gene",
    "parameters": ["db", "id", "retmode", "tool?", "email?"],
    "catalogue_digest": content_digest(dict(REVIEWED_NCBI_GENE_CATALOGUE)),
    "redirects": "refused", "credentials": "none",
})

_RETENTION = "bounded_gene_identity_alias_location_metadata_only"
_LIMITATIONS = (
    "the fixed human-gene catalogue is curated and is not an exhaustive gene search",
    "NCBI Gene metadata can change and does not establish disease relevance or evidence quality",
    "the adapter excludes Gene summaries, sequences, variants, expression, samples, and patient data",
    "retrieved symbols must match the reviewed catalogue exactly; changed records fail closed",
    "the adapter limits one request per execution and paces requests per process; deployments must coordinate rate limits across processes and hosts",
    "caller-injected transports own timeout, redirect, and network policy under their declared identity",
)
_SYMBOL_RE = re.compile(r"^[A-Z][A-Z0-9-]{0,15}$")
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_GENE_IDS = {symbol: gene_id for symbol, gene_id in REVIEWED_NCBI_GENE_CATALOGUE.items()}
_RATE_LOCK = threading.Lock()
_LAST_DISPATCH = 0.0


class ReviewedNcbiGeneRetrievalError(ArgumentError):
    """An NCBI Gene response or reviewed catalogue plan failed its bounded contract."""


GeneSummaryFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedNcbiGeneRetrievalError(message)


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ReviewedNcbiGeneRetrievalError(f"{name} is not bounded JSON") from error
    if len(encoded) > maximum:
        _fail(f"{name} exceeds its byte bound")
    return encoded


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        _fail(f"{name} is invalid")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its bounded integer range")
    return value


def _text(name: str, value: Any, maximum: int, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = " ".join(value.split())
    try:
        encoded = normalized.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ReviewedNcbiGeneRetrievalError(f"{name} contains invalid Unicode") from error
    if (not normalized and not optional) or len(encoded) > maximum or any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        _fail(f"{name} is empty or exceeds its text bound")
    return normalized or None


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(f"{name} must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedNcbiGeneRetrievalError(f"{name} is not a real UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _validate_tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_NCBI_GENE_TREE_NODES]
    budget[0] -= 1
    if budget[0] < 0 or depth > MAX_REVIEWED_NCBI_GENE_TREE_DEPTH:
        _fail("NCBI Gene response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("NCBI Gene response contains a non-text key")
            try:
                key.encode("utf-8")
            except UnicodeEncodeError as error:
                raise ReviewedNcbiGeneRetrievalError("NCBI Gene response contains invalid Unicode") from error
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, str):
        try:
            value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedNcbiGeneRetrievalError("NCBI Gene response contains invalid Unicode") from error
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("NCBI Gene response contains a non-finite number")
    elif value is not None and not isinstance(value, (str, int, float, bool)):
        _fail("NCBI Gene response contains an unsupported value")


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for key, value in pairs:
        if key in output:
            _fail("NCBI Gene response contains duplicate JSON fields")
        output[key] = value
    return output


def _parse_response(value: Any) -> tuple[Mapping[str, Any], int]:
    if isinstance(value, Mapping):
        encoded = _json_bytes(value, "NCBI Gene response", MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES)
        parsed: Any = value
    elif isinstance(value, str):
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedNcbiGeneRetrievalError("NCBI Gene response is not valid UTF-8") from error
        if len(encoded) > MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES:
            _fail("NCBI Gene response exceeds its byte bound")
        try:
            parsed = json.loads(encoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("NCBI Gene response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedNcbiGeneRetrievalError("NCBI Gene response is not valid JSON") from error
    elif isinstance(value, bytes):
        encoded = value
        if len(encoded) > MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES:
            _fail("NCBI Gene response exceeds its byte bound")
        try:
            parsed = json.loads(encoded.decode("utf-8"), object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("NCBI Gene response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedNcbiGeneRetrievalError("NCBI Gene response is not valid JSON") from error
    else:
        _fail("NCBI Gene response is not JSON")
    if not isinstance(parsed, Mapping):
        _fail("NCBI Gene response root is not an object")
    _validate_tree(parsed)
    return parsed, len(encoded)


def _registration(tool: Any, email: Any) -> tuple[str | None, str | None, str]:
    if (tool is None) != (email is None):
        _fail("NCBI tool and developer email must be provided together")
    if tool is None:
        return None, None, "none"
    if not isinstance(tool, str) or not re.fullmatch(r"[A-Za-z][A-Za-z0-9_.-]{0,63}", tool):
        _fail("NCBI tool must be a bounded application name without spaces")
    if not isinstance(email, str) or len(email) > 254 or not re.fullmatch(r"[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]+\.[A-Za-z]{2,63}", email) or not email.isascii():
        _fail("NCBI email must be a bounded developer email address")
    return tool, email, content_digest({"tool": tool, "email": email})


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _pace_process_requests() -> None:
    global _LAST_DISPATCH
    with _RATE_LOCK:
        remaining = 0.36 - (time.monotonic() - _LAST_DISPATCH)
        if remaining > 0:
            time.sleep(remaining)
        _LAST_DISPATCH = time.monotonic()


def _url(gene_ids: Sequence[str], tool: str | None, email: str | None) -> str:
    parameters: dict[str, str] = {"db": "gene", "id": ",".join(gene_ids), "retmode": "json"}
    if tool is not None and email is not None:
        parameters["tool"] = tool
        parameters["email"] = email
    url = f"{REVIEWED_NCBI_GENE_ENDPOINT}?{urlencode(parameters)}"
    if not url.startswith(f"{REVIEWED_NCBI_GENE_ENDPOINT}?") or len(url.encode("ascii")) > 8_192:
        _fail("NCBI Gene request escaped its reviewed endpoint")
    return url


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    if not url.startswith(f"{REVIEWED_NCBI_GENE_ENDPOINT}?"):
        _fail("NCBI Gene built-in transport refused an unpinned URL")
    request = Request(url, headers={"Accept": "application/json"}, method="GET")
    _pace_process_requests()
    try:
        with build_opener(_NoRedirect).open(request, timeout=timeout_ms / 1000) as response:
            if response.geturl() != url or response.status != 200:
                _fail("NCBI Gene endpoint returned an unexpected response")
            body = response.read(MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError) as error:
        raise ReviewedNcbiGeneRetrievalError("NCBI Gene request failed") from error
    if len(body) > MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES:
        _fail("NCBI Gene response exceeds its byte bound")
    return body


def _normalise_gene(payload: Mapping[str, Any], symbol: str) -> dict[str, Any]:
    gene_id = _GENE_IDS[symbol]
    result = payload.get("result")
    if not isinstance(result, Mapping) or result.get("uids") is None:
        _fail("NCBI Gene response omitted its result UID list")
    uids = result.get("uids")
    if not isinstance(uids, list) or not all(isinstance(uid, str) for uid in uids) or gene_id not in uids:
        _fail("NCBI Gene response UID coverage differs from the reviewed catalogue")
    raw = result.get(gene_id)
    if not isinstance(raw, Mapping) or raw.get("uid") != gene_id:
        _fail("NCBI Gene record identity does not match the reviewed GeneID")
    organism = raw.get("organism")
    if not isinstance(organism, Mapping) or organism.get("taxid") != 9606:
        _fail("NCBI Gene record is not the reviewed human organism")
    actual_symbol = _text("NCBI Gene symbol", raw.get("name"), 32)
    if actual_symbol != symbol:
        _fail("NCBI Gene symbol differs from the reviewed catalogue")
    aliases_raw = _text("NCBI Gene aliases", raw.get("otheraliases"), 16_000, optional=True)
    aliases: list[str] = []
    if aliases_raw:
        aliases = sorted({item for part in aliases_raw.split(",") if (item := _text("NCBI Gene alias", part, 256, optional=True))})
        if len(aliases) > 128:
            _fail("NCBI Gene aliases exceed their list bound")
    return {
        "source_id": "ncbi_gene", "gene_id": gene_id, "symbol": symbol,
        "description": _text("NCBI Gene description", raw.get("description"), 2_000),
        "chromosome": _text("NCBI Gene chromosome", raw.get("chromosome"), 64, optional=True),
        "map_location": _text("NCBI Gene map location", raw.get("maplocation"), 128, optional=True),
        "aliases": aliases, "organism_taxid": 9606,
    }


def _validate_response_coverage(payload: Mapping[str, Any], symbols: Sequence[str]) -> None:
    header = payload.get("header")
    if not isinstance(header, Mapping) or header.get("type") != "esummary":
        _fail("NCBI Gene response is not an ESummary result")
    result = payload.get("result")
    if not isinstance(result, Mapping):
        _fail("NCBI Gene response omitted its result object")
    expected_ids = [_GENE_IDS[symbol] for symbol in symbols]
    if result.get("uids") != expected_ids:
        _fail("NCBI Gene response is incomplete or reordered against the reviewed catalogue")
    if set(result) != {"uids", *expected_ids}:
        _fail("NCBI Gene response contains unrequested records")
    for symbol in symbols:
        _normalise_gene(payload, symbol)


@dataclass(frozen=True, slots=True)
class ReviewedNcbiGeneRetrievalConfig:
    gene_symbols: tuple[str, ...] = ("IDH1", "IDH2", "MGMT", "EGFR", "TERT", "TP53", "ATRX", "NF1", "PTEN", "CDKN2A", "PDGFRA", "BRAF")
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_NCBI_GENE_TRANSPORT_ID
    transport_version: str = BUILTIN_NCBI_GENE_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST
    ncbi_tool: str | None = field(default=None, repr=False, compare=False)
    ncbi_email: str | None = field(default=None, repr=False, compare=False)
    _registration_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.gene_symbols, Sequence) or isinstance(self.gene_symbols, (str, bytes)):
            _fail("NCBI Gene symbols must be a bounded catalogue list")
        symbols = tuple(self.gene_symbols)
        if not symbols or len(symbols) > MAX_REVIEWED_NCBI_GENE_SYMBOLS or any(not isinstance(symbol, str) or symbol not in REVIEWED_NCBI_GENE_CATALOGUE for symbol in symbols) or len(set(symbols)) != len(symbols):
            _fail("NCBI Gene symbols contain an unsupported or duplicate catalogue entry")
        normalized = tuple(symbol for symbol in REVIEWED_NCBI_GENE_CATALOGUE if symbol in symbols)
        object.__setattr__(self, "gene_symbols", normalized)
        object.__setattr__(self, "timeout_ms", _integer("NCBI Gene timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER_RE.fullmatch(value):
                _fail(f"NCBI Gene {name} is invalid")
        _digest("NCBI Gene transport_config_digest", self.transport_config_digest)
        _tool, _email, registration_digest = _registration(self.ncbi_tool, self.ncbi_email)
        if self._registration_digest not in ("", "none") and _digest("NCBI Gene registration_digest", self._registration_digest) != registration_digest:
            _fail("NCBI Gene registration identity changed")
        object.__setattr__(self, "_registration_digest", registration_digest)
        _json_bytes(self.to_dict(), "NCBI Gene config", 32_000)

    @property
    def request_limit(self) -> int:
        return 1

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_NCBI_GENE_CONFIG_SCHEMA, "gene_symbols": list(self.gene_symbols),
            "gene_ids": [_GENE_IDS[symbol] for symbol in self.gene_symbols], "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit, "transport_id": self.transport_id,
            "transport_version": self.transport_version, "transport_config_digest": self.transport_config_digest,
            "catalogue_digest": content_digest(dict(REVIEWED_NCBI_GENE_CATALOGUE)),
            "ncbi_registration_digest": self._registration_digest, "retention": _RETENTION,
            "credentials": "none",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any], *, ncbi_tool: str | None = None, ncbi_email: str | None = None) -> "ReviewedNcbiGeneRetrievalConfig":
        expected = {"schema", "gene_symbols", "gene_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "catalogue_digest", "ncbi_registration_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_NCBI_GENE_CONFIG_SCHEMA:
            _fail("NCBI Gene config has an invalid shape")
        config = cls(tuple(raw["gene_symbols"]), raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], ncbi_tool, ncbi_email, raw["ncbi_registration_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("NCBI Gene config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedNcbiGeneRetrievalPlan:
    config: ReviewedNcbiGeneRetrievalConfig
    config_digest: str
    catalogue_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedNcbiGeneRetrievalConfig) -> "ReviewedNcbiGeneRetrievalPlan":
        if type(config) is not ReviewedNcbiGeneRetrievalConfig:
            _fail("NCBI Gene plan requires an exact config")
        unsigned = {"schema": REVIEWED_NCBI_GENE_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "catalogue_digest": content_digest(dict(REVIEWED_NCBI_GENE_CATALOGUE)), "request_limit": 1, "scope": "fixed_human_ncbi_gene_summary_metadata", "execution": "one_bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "none"}
        return cls(config, config.config_digest, unsigned["catalogue_digest"], content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {"schema": REVIEWED_NCBI_GENE_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "catalogue_digest": self.catalogue_digest, "request_limit": 1, "scope": "fixed_human_ncbi_gene_summary_metadata", "execution": "one_bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "none"}
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if (self.config_digest, self.catalogue_digest, self.plan_digest) != (expected.config_digest, expected.catalogue_digest, expected.plan_digest):
            _fail("NCBI Gene plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any], *, ncbi_tool: str | None = None, ncbi_email: str | None = None) -> "ReviewedNcbiGeneRetrievalPlan":
        expected = {"schema", "config", "config_digest", "catalogue_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_NCBI_GENE_PLAN_SCHEMA:
            _fail("NCBI Gene plan has an invalid shape")
        plan = cls.create(ReviewedNcbiGeneRetrievalConfig.from_dict(raw["config"], ncbi_tool=ncbi_tool, ncbi_email=ncbi_email))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("NCBI Gene plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedNcbiGeneRetrievalResult:
    _bundle: Mapping[str, Any] = field(repr=False)
    _receipt: Mapping[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(dict(self._bundle))

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(dict(self._receipt))

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": self.receipt, "retention": "metadata_only"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_gene_metadata"}


class ReviewedNcbiGeneRetrievalAdapter:
    """Read a fixed NCBI GeneID catalogue after explicit caller review."""

    def __init__(self, config: ReviewedNcbiGeneRetrievalConfig, *, fetch: GeneSummaryFetcher | None = None) -> None:
        if type(config) is not ReviewedNcbiGeneRetrievalConfig:
            _fail("NCBI Gene adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("NCBI Gene injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_NCBI_GENE_TRANSPORT_ID:
            _fail("NCBI Gene injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id != BUILTIN_NCBI_GENE_TRANSPORT_ID or config.transport_config_digest != BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST):
            _fail("NCBI Gene built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedNcbiGeneRetrievalPlan:
        return ReviewedNcbiGeneRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedNcbiGeneRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedNcbiGeneRetrievalResult:
        if type(plan) is not ReviewedNcbiGeneRetrievalPlan:
            _fail("NCBI Gene execution requires an exact reviewed plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("NCBI Gene execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("NCBI Gene dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "NCBI Gene retrieved_at")
        gene_ids = [_GENE_IDS[symbol] for symbol in self.config.gene_symbols]
        tool, email, _registration_digest = _registration(self.config.ncbi_tool, self.config.ncbi_email)
        url = _url(gene_ids, tool, email)
        try:
            if self._fetch is None:
                raw = _builtin_fetch(url, self.config.timeout_ms)
            else:
                _pace_process_requests()
                raw = self._fetch(url)
        except ReviewedNcbiGeneRetrievalError:
            raise
        except Exception as error:
            raise ReviewedNcbiGeneRetrievalError("NCBI Gene request failed") from error
        payload, response_bytes = _parse_response(raw)
        _validate_response_coverage(payload, self.config.gene_symbols)
        genes = [_normalise_gene(payload, symbol) for symbol in self.config.gene_symbols]
        content_sha256 = content_digest(genes)
        source = {"source_id": "ncbi_gene", "authority": REVIEWED_NCBI_GENE_AUTHORITY, "uri": f"https://www.ncbi.nlm.nih.gov/gene/?term={','.join(gene_ids)}", "retrieved_at": timestamp, "content_sha256": content_sha256, "record_count": len(genes), "provider": "none", "credentials": "none", "limitations": list(_LIMITATIONS)}
        source_digest = content_digest([source])
        bundle_unsigned = {"schema": REVIEWED_NCBI_GENE_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": [source], "genes": genes, "source_set_digest": source_digest, "gene_count": len(genes), "catalogue_coverage": "complete", "provider": "none", "credentials": "none", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "NCBI Gene bundle", MAX_REVIEWED_NCBI_GENE_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        receipt_unsigned = {"schema": REVIEWED_NCBI_GENE_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "catalogue_digest": plan.catalogue_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_digest, "source_count": 1, "gene_count": len(genes), "request_count": 1, "response_bytes": response_bytes, "catalogue_coverage": "complete", "retrieved_at": timestamp, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "none", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedNcbiGeneRetrievalResult(bundle, receipt)


def create_reviewed_ncbi_gene_execution_metadata(plan: ReviewedNcbiGeneRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedNcbiGeneRetrievalPlan:
        _fail("NCBI Gene execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("NCBI Gene execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "NCBI Gene retrieved_at")
    payload = {"schema": REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "none"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedNcbiGeneRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    if not isinstance(value, Mapping) or set(value) != {"schema", "bundle", "receipt", "retention"} or value.get("schema") != REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA or value.get("retention") != "caller_owned_transient_gene_metadata":
        _fail("NCBI Gene transient value is malformed")
    bundle, receipt = value["bundle"], value["receipt"]
    if not isinstance(bundle, Mapping) or not isinstance(receipt, Mapping):
        _fail("NCBI Gene transient bundle or receipt is malformed")
    bundle_keys = {"schema", "generated_at", "sources", "genes", "source_set_digest", "gene_count", "catalogue_coverage", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_keys = {"schema", "plan_digest", "config_digest", "catalogue_digest", "bundle_digest", "source_set_digest", "source_count", "gene_count", "request_count", "response_bytes", "catalogue_coverage", "retrieved_at", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if set(bundle) != bundle_keys or set(receipt) != receipt_keys:
        _fail("NCBI Gene transient bundle or receipt has an unexpected shape")
    bundle_unsigned = {key: item for key, item in bundle.items() if key != "bundle_digest"}
    receipt_unsigned = {key: item for key, item in receipt.items() if key != "receipt_digest"}
    if content_digest(bundle_unsigned) != _digest("NCBI Gene bundle_digest", bundle.get("bundle_digest")) or content_digest(receipt_unsigned) != _digest("NCBI Gene receipt_digest", receipt.get("receipt_digest")):
        _fail("NCBI Gene transient digests are invalid")
    if bundle.get("schema") != REVIEWED_NCBI_GENE_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_NCBI_GENE_RECEIPT_SCHEMA or receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("catalogue_digest") != plan.catalogue_digest:
        _fail("NCBI Gene transient identity differs from the reviewed plan")
    if receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("provider") != "none" or receipt.get("credentials") != "none" or receipt.get("retention") != _RETENTION or bundle.get("provider") != "none" or bundle.get("credentials") != "none":
        _fail("NCBI Gene transient boundary metadata is invalid")
    if canonical_json(bundle.get("limitations")) != canonical_json(list(_LIMITATIONS)) or canonical_json(receipt.get("limitations")) != canonical_json(list(_LIMITATIONS)):
        _fail("NCBI Gene transient limitations are invalid")
    timestamp = _timestamp(bundle.get("generated_at"), "NCBI Gene bundle timestamp")
    if timestamp != _timestamp(receipt.get("retrieved_at"), "NCBI Gene receipt timestamp"):
        _fail("NCBI Gene bundle and receipt timestamps do not match")
    genes, sources = bundle.get("genes"), bundle.get("sources")
    if not isinstance(genes, list) or not isinstance(sources, list) or len(genes) != len(plan.config.gene_symbols) or len(sources) != 1:
        _fail("NCBI Gene transient catalogue coverage is incomplete")
    normalized = []
    for symbol, gene in zip(plan.config.gene_symbols, genes):
        if not isinstance(gene, Mapping) or set(gene) != {"source_id", "gene_id", "symbol", "description", "chromosome", "map_location", "aliases", "organism_taxid"}:
            _fail("NCBI Gene transient record has an invalid shape")
        expected = {"source_id": "ncbi_gene", "gene_id": _GENE_IDS[symbol], "symbol": symbol,
                    "description": _text("NCBI Gene description", gene.get("description"), 2_000),
                    "chromosome": _text("NCBI Gene chromosome", gene.get("chromosome"), 64, optional=True),
                    "map_location": _text("NCBI Gene map location", gene.get("map_location"), 128, optional=True),
                    "aliases": gene.get("aliases"), "organism_taxid": 9606}
        aliases = gene.get("aliases")
        if not isinstance(aliases, list) or len(aliases) > 128 or aliases != sorted(set(aliases)) or any(_text("NCBI Gene alias", alias, 256) != alias for alias in aliases):
            _fail("NCBI Gene transient aliases are malformed")
        if gene.get("organism_taxid") != 9606 or canonical_json(gene) != canonical_json(expected):
            _fail("NCBI Gene transient record differs from its reviewed catalogue")
        normalized.append(expected)
    source = sources[0]
    if not isinstance(source, Mapping) or set(source) != {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"}:
        _fail("NCBI Gene transient source metadata is malformed")
    genes_digest = content_digest(normalized)
    expected_source = {"source_id": "ncbi_gene", "authority": REVIEWED_NCBI_GENE_AUTHORITY, "uri": f"https://www.ncbi.nlm.nih.gov/gene/?term={','.join(_GENE_IDS[symbol] for symbol in plan.config.gene_symbols)}", "retrieved_at": timestamp, "content_sha256": genes_digest, "record_count": len(genes), "provider": "none", "credentials": "none", "limitations": list(_LIMITATIONS)}
    if canonical_json(source) != canonical_json(expected_source) or bundle.get("source_set_digest") != content_digest([expected_source]) or receipt.get("source_set_digest") != content_digest([expected_source]) or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("NCBI Gene transient source binding is invalid")
    if receipt.get("source_count") != 1 or receipt.get("gene_count") != len(genes) or bundle.get("gene_count") != len(genes) or receipt.get("request_count") != 1 or receipt.get("catalogue_coverage") != "complete" or bundle.get("catalogue_coverage") != "complete":
        _fail("NCBI Gene transient counts or coverage are inconsistent")
    _integer("NCBI Gene response_bytes", receipt.get("response_bytes"), 1, MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES)
    return bundle, receipt


def create_reviewed_ncbi_gene_autonomous_evidence_registration(adapter: ReviewedNcbiGeneRetrievalAdapter, plan: ReviewedNcbiGeneRetrievalPlan) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedNcbiGeneRetrievalAdapter or type(plan) is not ReviewedNcbiGeneRetrievalPlan:
        _fail("NCBI Gene registration requires exact adapter and plan values")
    plan.validate()
    if canonical_json(adapter.config.to_dict()) != canonical_json(plan.config.to_dict()):
        _fail("NCBI Gene registration config differs from its reviewed plan")
    frozen_plan = ReviewedNcbiGeneRetrievalPlan.create(plan.config)
    expected_network = "builtin_https" if adapter._fetch is None else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != "ncbi_gene" or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("NCBI Gene acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected:
            _fail("NCBI Gene acquisition metadata is malformed")
        unsigned = dict(metadata)
        supplied = _digest("NCBI Gene metadata_digest", unsigned.pop("metadata_digest"))
        if supplied != content_digest(unsigned) or metadata.get("schema") != REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "none":
            _fail("NCBI Gene acquisition metadata failed review binding")
        retrieved_at = None if metadata["retrieved_at"] is None else _timestamp(metadata["retrieved_at"], "NCBI Gene retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=retrieved_at).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("NCBI Gene projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(adapter_id="reviewed.ncbi_gene", version=REVIEWED_NCBI_GENE_ADAPTER_VERSION, domains=("biomedical", "neuroscience"), capabilities=("gene_catalogue_metadata", "source_provenance"), source_kinds=("ncbi_gene_summary_metadata",), acquire=acquire, project=project)


__all__ = [name for name in globals() if name.startswith("REVIEWED_NCBI_GENE_") or name.startswith("MAX_REVIEWED_NCBI_GENE_") or name.startswith("BUILTIN_NCBI_GENE_")] + [
    "GeneSummaryFetcher", "ReviewedNcbiGeneRetrievalError", "ReviewedNcbiGeneRetrievalConfig", "ReviewedNcbiGeneRetrievalPlan", "ReviewedNcbiGeneRetrievalResult", "ReviewedNcbiGeneRetrievalAdapter", "create_reviewed_ncbi_gene_execution_metadata", "create_reviewed_ncbi_gene_autonomous_evidence_registration",
]
