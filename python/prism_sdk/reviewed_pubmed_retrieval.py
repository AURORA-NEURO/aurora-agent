"""Reviewed, bounded PubMed retrieval for the generic autonomous evidence plane.

This is a deployment-owned adapter, not a new scientific kernel or a claim that retrieved
citations are sufficient evidence.  Preparation is a pure operation over the six fixed
neurosurgical specialty lanes.  Execution requires a literal approval value, revalidates the
exact reviewed plan and captured transport immediately before every request, and delegates
parsing plus Rust-compatible bundle hashing to :mod:`prism_sdk.public_literature_refresh`.

Only a metadata receipt is durable.  Query strings, response bodies, and the transient literature
bundle stay on the caller-owned return path.  No credential argument and no synthetic fallback
exist.  This closes one deployment gap described by the autonomous-brain backlog; authenticated
shared coordination, uncertain-call reconciliation, and independent evidence-quality review are
still outside this module.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from datetime import datetime
import json
import math
import re
import time
from types import MethodType
from typing import Any
from urllib.parse import parse_qsl, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener
import xml.etree.ElementTree as ET

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration
from . import public_literature_refresh as _public_literature_module
from .public_literature_refresh import (
    MAX_PER_SPECIALTY_LIMIT,
    MAX_PUBMED_LANES,
    MAX_RESPONSE_BYTES,
    PUBMED_AUTHORITY,
    PUBMED_SPECIALTY_LANES,
    PUBLIC_LITERATURE_SCHEMA_VERSION,
    PubMedFetcher,
    PublicLiteratureRefreshError,
    _ncbi_registration_parameters as _PUBLIC_NCBI_REGISTRATION_PARAMETERS,
    bundle_digest,
    refresh_neurosurgical_public_literature as _PUBLIC_REFRESH,
    validate_public_literature_bundle,
)


REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA = (
    "bioprism-python-reviewed-pubmed-retrieval-config/0.1"
)
REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA = (
    "bioprism-python-reviewed-pubmed-retrieval-plan/0.1"
)
REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA = (
    "bioprism-python-reviewed-pubmed-source-receipt/0.1"
)
REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA = (
    "bioprism-python-reviewed-pubmed-retrieval-receipt/0.1"
)
REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA = (
    "bioprism-python-reviewed-pubmed-transient-value/0.1"
)
REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA = (
    "bioprism-python-reviewed-pubmed-execution-metadata/0.1"
)
REVIEWED_PUBMED_QUERY_SET_SCHEMA = "bioprism-python-reviewed-pubmed-query-set/0.1"
REVIEWED_PUBMED_NCBI_REGISTRATION_SCHEMA = (
    "bioprism-python-reviewed-pubmed-ncbi-registration/0.1"
)

REVIEWED_PUBMED_ADAPTER_VERSION = "0.1"
REVIEWED_PUBMED_ENDPOINTS = ("esearch.fcgi", "esummary.fcgi", "efetch.fcgi")
REVIEWED_PUBMED_HOST = "eutils.ncbi.nlm.nih.gov"
MAX_REVIEWED_PUBMED_REQUESTS = MAX_PUBMED_LANES * len(REVIEWED_PUBMED_ENDPOINTS)
MAX_REVIEWED_PUBMED_RECORDS = MAX_PUBMED_LANES * MAX_PER_SPECIALTY_LIMIT
MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES = 64_000_000
MAX_REVIEWED_PUBMED_BUNDLE_BYTES = 8_000_000
MAX_REVIEWED_PUBMED_RESPONSE_DEPTH = 32
MAX_REVIEWED_PUBMED_RESPONSE_NODES = 100_000
MAX_REVIEWED_PUBMED_ARTIFACT_BYTES = 64_000

BUILTIN_PUBMED_TRANSPORT_ID = "builtin.ncbi_eutils.urllib"
BUILTIN_PUBMED_TRANSPORT_VERSION = "1"
BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST = content_digest(
    {
        "implementation": "python_stdlib_urllib",
        "method": "GET",
        "scheme": "https",
        "host": REVIEWED_PUBMED_HOST,
        "paths": [
            f"/entrez/eutils/{endpoint}" for endpoint in REVIEWED_PUBMED_ENDPOINTS
        ],
        "redirects": "refused",
        "rate_limit": "at_most_three_requests_per_second",
        "request_body": "none",
        "registration_parameters": "optional_registered_tool_and_developer_email",
        "secret_material": "not_accepted",
    }
)

_CANONICAL_SPECIALTY_QUERY_TERMS = tuple(PUBMED_SPECIALTY_LANES.items())
_CANONICAL_SPECIALTY_LANES = tuple(
    lane for lane, _term in _CANONICAL_SPECIALTY_QUERY_TERMS
)

_DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:+-]+$")
_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
_PUBMED_DOCTYPE_RE = re.compile(
    rb"<!DOCTYPE\s+PubmedArticleSet\s+PUBLIC\s+"
    rb'"-//NLM//DTD PubMedArticle,\s+[0-9A-Za-z ]{1,48}//EN"\s+'
    rb'"https://dtd\.nlm\.nih\.gov/ncbi/pubmed/out/pubmed_[0-9]{6}\.dtd"\s*>'
)
_PUBMED_XML_PREFIX_RE = re.compile(
    rb'^(?:\xef\xbb\xbf)?[ \t\r\n]*(?:<\?xml\s+version=["\']1\.0["\']'
    rb'(?:\s+encoding=["\']utf-8["\'])?(?:\s+standalone=["\'](?:yes|no)["\'])?\s*\?>'
    rb"[ \t\r\n]*)?$",
    re.IGNORECASE,
)
_CONFIG_KEYS = frozenset(
    {
        "schema",
        "specialty_lanes",
        "per_specialty_limit",
        "timeout_seconds",
        "request_limit",
        "record_limit",
        "response_byte_limit",
        "total_response_byte_limit",
        "bundle_byte_limit",
        "transport_id",
        "transport_version",
        "transport_config_digest",
        "query_set_digest",
        "ncbi_registration_configured",
        "ncbi_registration_digest",
        "scope",
        "execution",
        "retention",
        "secret_material",
        "config_digest",
    }
)
_PLAN_KEYS = frozenset(
    {
        "schema",
        "status",
        "config_digest",
        "specialty_lanes",
        "per_specialty_limit",
        "request_limit",
        "record_limit",
        "response_byte_limit",
        "total_response_byte_limit",
        "bundle_byte_limit",
        "transport_id",
        "transport_version",
        "transport_config_digest",
        "query_set_digest",
        "ncbi_registration_configured",
        "ncbi_registration_digest",
        "authority",
        "scope",
        "execution",
        "retention",
        "secret_material",
        "limitations",
        "plan_digest",
    }
)
_SOURCE_RECEIPT_KEYS = frozenset(
    {"schema", "specialty_lane", "source_id", "content_digest", "record_count"}
)
_RECEIPT_KEYS = frozenset(
    {
        "schema",
        "plan_digest",
        "config_digest",
        "specialty_lanes",
        "transport_id",
        "transport_version",
        "transport_config_digest",
        "query_set_digest",
        "ncbi_registration_configured",
        "ncbi_registration_digest",
        "generated_at",
        "bundle_schema",
        "bundle_digest",
        "source_set_digest",
        "sources",
        "source_count",
        "record_count",
        "abstract_count",
        "request_count",
        "response_bytes",
        "synthetic_data",
        "human_review_required",
        "retention",
        "secret_material",
        "limitations",
        "receipt_digest",
    }
)
_TRANSIENT_VALUE_KEYS = frozenset({"schema", "lane", "bundle", "receipt", "retention"})
_EXECUTION_METADATA_KEYS = frozenset(
    {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at"}
)
_BUNDLE_KEYS = frozenset(
    {"schema_version", "generated_at", "synthetic_data", "sources", "records"}
)
_BUNDLE_SOURCE_KEYS = frozenset(
    {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count"}
)
_BUNDLE_RECORD_KEYS = frozenset(
    {
        "source_id",
        "specialty",
        "pmid",
        "title",
        "journal",
        "publication_date",
        "doi",
        "abstract_text",
        "abstract_truncated",
        "publication_types",
        "mesh_terms",
    }
)
_CONFIG_EXECUTION = "review_configuration_only;no_source_dispatch"
_PLAN_EXECUTION = "explicit_literal_approval_required;bounded_public_https_get_only"
_CONFIG_RETENTION = "metadata_only;query_transport_and_ncbi_contact_values_excluded"
_RECEIPT_RETENTION = (
    "metadata_only;transient_bundle_transport_and_ncbi_contact_values_excluded"
)
_TRANSIENT_RETENTION = (
    "caller_owned_transient_value;do_not_persist_without_separate_policy"
)
_SECRET_MATERIAL = "api_keys_and_secrets_never_accepted_or_returned"
_LIMITATIONS = (
    "PubMed metadata and abstracts are source text, not verified scientific conclusions",
    "retrieval coverage is limited to the selected fixed specialty lanes and record window",
    "deduplication is PMID-only and partial publication dates remain unknown",
    "a qualified reviewer must assess omissions, study quality, freshness, and applicability",
    "tool and developer email values must be registered separately with NCBI before use",
    "the registration digest is an integrity binding, not anonymization of guessable contact values",
    "same-process callable behavior beyond captured identity and function code is caller-controlled",
)


class ReviewedPubMedRetrievalError(PublicLiteratureRefreshError):
    """A reviewed PubMed artifact or dispatch violated its bounded contract."""


def _plan_payload(values: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "schema": REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA,
        "status": "ready_for_review",
        "config_digest": values["config_digest"],
        "specialty_lanes": list(values["specialty_lanes"]),
        "per_specialty_limit": values["per_specialty_limit"],
        "request_limit": values["request_limit"],
        "record_limit": values["record_limit"],
        "response_byte_limit": values["response_byte_limit"],
        "total_response_byte_limit": values["total_response_byte_limit"],
        "bundle_byte_limit": values["bundle_byte_limit"],
        "transport_id": values["transport_id"],
        "transport_version": values["transport_version"],
        "transport_config_digest": values["transport_config_digest"],
        "query_set_digest": values["query_set_digest"],
        "ncbi_registration_configured": values["ncbi_registration_configured"],
        "ncbi_registration_digest": values["ncbi_registration_digest"],
        "authority": PUBMED_AUTHORITY,
        "scope": _scope(),
        "execution": _PLAN_EXECUTION,
        "retention": _CONFIG_RETENTION,
        "secret_material": _SECRET_MATERIAL,
        "limitations": list(_LIMITATIONS),
    }


def _receipt_payload(values: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "schema": REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA,
        "plan_digest": values["plan_digest"],
        "config_digest": values["config_digest"],
        "specialty_lanes": list(values["specialty_lanes"]),
        "transport_id": values["transport_id"],
        "transport_version": values["transport_version"],
        "transport_config_digest": values["transport_config_digest"],
        "query_set_digest": values["query_set_digest"],
        "ncbi_registration_configured": values["ncbi_registration_configured"],
        "ncbi_registration_digest": values["ncbi_registration_digest"],
        "generated_at": values["generated_at"],
        "bundle_schema": values["bundle_schema"],
        "bundle_digest": values["bundle_digest"],
        "source_set_digest": values["source_set_digest"],
        "sources": [source.to_dict() for source in values["sources"]],
        "source_count": values["source_count"],
        "record_count": values["record_count"],
        "abstract_count": values["abstract_count"],
        "request_count": values["request_count"],
        "response_bytes": values["response_bytes"],
        "synthetic_data": False,
        "human_review_required": True,
        "retention": _RECEIPT_RETENTION,
        "secret_material": _SECRET_MATERIAL,
        "limitations": list(_LIMITATIONS),
    }


def _exact_dict(value: Any, keys: frozenset[str], name: str) -> dict[str, Any]:
    if type(value) is not dict or set(value) != keys:
        raise ReviewedPubMedRetrievalError(
            f"{name} must contain exactly its schema fields"
        )
    return value


def _text(value: Any, name: str, maximum: int = 512) -> str:
    if (
        type(value) is not str
        or not value.strip()
        or value != value.strip()
        or "\x00" in value
        or len(value.encode("utf-8")) > maximum
    ):
        raise ReviewedPubMedRetrievalError(
            f"{name} is outside its bounded text contract"
        )
    return value


def _identifier(value: Any, name: str, maximum: int = 256) -> str:
    result = _text(value, name, maximum)
    if _IDENTIFIER_RE.fullmatch(result) is None:
        raise ReviewedPubMedRetrievalError(f"{name} is outside its identifier contract")
    return result


def _digest(value: Any, name: str) -> str:
    if type(value) is not str or _DIGEST_RE.fullmatch(value) is None:
        raise ReviewedPubMedRetrievalError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(value: Any, name: str, minimum: int, maximum: int) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise ReviewedPubMedRetrievalError(
            f"{name} must be an integer between {minimum} and {maximum}"
        )
    return value


def _timeout(value: Any) -> float:
    if (
        type(value) not in {int, float}
        or isinstance(value, bool)
        or not math.isfinite(float(value))
    ):
        raise ReviewedPubMedRetrievalError("timeout_seconds must be a finite number")
    result = float(value)
    if not 1 <= result <= 120:
        raise ReviewedPubMedRetrievalError("timeout_seconds must be between 1 and 120")
    return result


def _timestamp(value: Any, name: str = "generated_at") -> str:
    if type(value) is not str or _UTC_RE.fullmatch(value) is None:
        raise ReviewedPubMedRetrievalError(
            f"{name} must be a whole-second UTC timestamp"
        )
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedPubMedRetrievalError(
            f"{name} must be a valid UTC timestamp"
        ) from error
    return value


def _lanes(value: Any, name: str = "specialty_lanes") -> tuple[str, ...]:
    if type(value) not in {list, tuple}:
        raise ReviewedPubMedRetrievalError(f"{name} must be an exact list or tuple")
    requested = tuple(value)
    if not 1 <= len(requested) <= MAX_PUBMED_LANES:
        raise ReviewedPubMedRetrievalError(
            f"{name} must contain 1..{MAX_PUBMED_LANES} lanes"
        )
    if any(
        type(lane) is not str or lane not in _CANONICAL_SPECIALTY_LANES
        for lane in requested
    ):
        raise ReviewedPubMedRetrievalError(f"{name} contains a non-allow-listed lane")
    if len(set(requested)) != len(requested):
        raise ReviewedPubMedRetrievalError(f"{name} contains duplicate lanes")
    selected = set(requested)
    return tuple(lane for lane in _CANONICAL_SPECIALTY_LANES if lane in selected)


def _query_terms_for(lanes: Sequence[str]) -> tuple[tuple[str, str], ...]:
    selected = set(lanes)
    return tuple(
        (lane, term)
        for lane, term in _CANONICAL_SPECIALTY_QUERY_TERMS
        if lane in selected
    )


def _query_set_digest(query_terms: Sequence[tuple[str, str]]) -> str:
    return content_digest(
        {
            "schema": REVIEWED_PUBMED_QUERY_SET_SCHEMA,
            "queries": [
                {"specialty_lane": lane, "query_digest": content_digest({"term": term})}
                for lane, term in query_terms
            ],
        }
    )


def _ncbi_registration(
    ncbi_tool: str | None,
    ncbi_email: str | None,
) -> tuple[tuple[str, str], ...]:
    """Return the exact optional NCBI registration pair without retaining a mutable mapping."""

    try:
        parameters = _PUBLIC_NCBI_REGISTRATION_PARAMETERS(ncbi_tool, ncbi_email)
    except PublicLiteratureRefreshError as error:
        raise ReviewedPubMedRetrievalError(str(error)) from error
    if type(parameters) is not dict or tuple(parameters) not in {(), ("tool", "email")}:
        raise ReviewedPubMedRetrievalError(
            "NCBI registration validator returned an invalid parameter set"
        )
    return tuple(parameters.items())


def _ncbi_registration_digest(registration: Sequence[tuple[str, str]]) -> str:
    return content_digest(
        {
            "schema": REVIEWED_PUBMED_NCBI_REGISTRATION_SCHEMA,
            "configured": bool(registration),
            "parameters": [
                {"name": name, "value": value} for name, value in registration
            ],
        }
    )


_UNCONFIGURED_NCBI_REGISTRATION_DIGEST = _ncbi_registration_digest(())


def _scope() -> dict[str, Any]:
    return {
        "scheme": "https",
        "host": REVIEWED_PUBMED_HOST,
        "paths": [
            f"/entrez/eutils/{endpoint}" for endpoint in REVIEWED_PUBMED_ENDPOINTS
        ],
        "method": "GET",
        "request_body": "none",
    }


def _bounded_artifact(
    value: Any, name: str, maximum: int = MAX_REVIEWED_PUBMED_ARTIFACT_BYTES
) -> int:
    try:
        size = len(canonical_json(value).encode("utf-8"))
    except (TypeError, ValueError) as error:
        raise ReviewedPubMedRetrievalError(f"{name} must be canonical JSON") from error
    if size > maximum:
        raise ReviewedPubMedRetrievalError(f"{name} exceeds its byte bound")
    return size


@dataclass(frozen=True, slots=True)
class ReviewedPubMedRetrievalConfig:
    """Immutable reviewed scope and transport identity; constructing it performs no I/O.

    Optional NCBI ``tool`` and developer ``email`` values are excluded from ``repr`` and
    :meth:`to_dict`; only their integrity digest and configured state enter durable artifacts.
    They are request identification, not credentials, and must already be registered with NCBI.
    """

    specialty_lanes: tuple[str, ...]
    per_specialty_limit: int = 10
    timeout_seconds: float = 30.0
    response_byte_limit: int = MAX_RESPONSE_BYTES
    total_response_byte_limit: int = 48_000_000
    bundle_byte_limit: int = MAX_REVIEWED_PUBMED_BUNDLE_BYTES
    transport_id: str = BUILTIN_PUBMED_TRANSPORT_ID
    transport_version: str = BUILTIN_PUBMED_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST
    query_set_digest: str | None = None
    ncbi_tool: str | None = field(default=None, repr=False, compare=False)
    ncbi_email: str | None = field(default=None, repr=False, compare=False)
    ncbi_registration_digest: str | None = None

    def __post_init__(self) -> None:
        normalized_lanes = _lanes(self.specialty_lanes)
        per_lane = _integer(
            self.per_specialty_limit,
            "per_specialty_limit",
            1,
            MAX_PER_SPECIALTY_LIMIT,
        )
        timeout = _timeout(self.timeout_seconds)
        response_limit = _integer(
            self.response_byte_limit,
            "response_byte_limit",
            256,
            MAX_RESPONSE_BYTES,
        )
        total_limit = _integer(
            self.total_response_byte_limit,
            "total_response_byte_limit",
            response_limit,
            MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES,
        )
        bundle_limit = _integer(
            self.bundle_byte_limit,
            "bundle_byte_limit",
            1_024,
            MAX_REVIEWED_PUBMED_BUNDLE_BYTES,
        )
        object.__setattr__(self, "specialty_lanes", normalized_lanes)
        object.__setattr__(self, "per_specialty_limit", per_lane)
        object.__setattr__(self, "timeout_seconds", timeout)
        object.__setattr__(self, "response_byte_limit", response_limit)
        object.__setattr__(self, "total_response_byte_limit", total_limit)
        object.__setattr__(self, "bundle_byte_limit", bundle_limit)
        object.__setattr__(
            self, "transport_id", _identifier(self.transport_id, "transport_id")
        )
        object.__setattr__(
            self,
            "transport_version",
            _identifier(self.transport_version, "transport_version"),
        )
        object.__setattr__(
            self,
            "transport_config_digest",
            _digest(self.transport_config_digest, "transport_config_digest"),
        )
        expected_query_set_digest = _query_set_digest(
            _query_terms_for(normalized_lanes)
        )
        if (
            self.query_set_digest is not None
            and _digest(self.query_set_digest, "query_set_digest")
            != expected_query_set_digest
        ):
            raise ReviewedPubMedRetrievalError(
                "query_set_digest does not match the fixed selected-lane queries"
            )
        object.__setattr__(self, "query_set_digest", expected_query_set_digest)
        registration = _ncbi_registration(self.ncbi_tool, self.ncbi_email)
        expected_registration_digest = _ncbi_registration_digest(registration)
        if (
            self.ncbi_registration_digest is not None
            and _digest(self.ncbi_registration_digest, "ncbi_registration_digest")
            != expected_registration_digest
        ):
            raise ReviewedPubMedRetrievalError(
                "ncbi_registration_digest does not match the configured NCBI contact pair"
            )
        object.__setattr__(
            self, "ncbi_registration_digest", expected_registration_digest
        )
        _bounded_artifact(self.to_dict(), "reviewed PubMed retrieval config")

    @property
    def ncbi_registration_configured(self) -> bool:
        return bool(_ncbi_registration(self.ncbi_tool, self.ncbi_email))

    @property
    def request_limit(self) -> int:
        return len(self.specialty_lanes) * len(REVIEWED_PUBMED_ENDPOINTS)

    @property
    def record_limit(self) -> int:
        return len(self.specialty_lanes) * self.per_specialty_limit

    def _payload(self) -> dict[str, Any]:
        registration = _ncbi_registration(self.ncbi_tool, self.ncbi_email)
        if _ncbi_registration_digest(registration) != self.ncbi_registration_digest:
            raise ReviewedPubMedRetrievalError(
                "configured NCBI contact pair changed after review"
            )
        return {
            "schema": REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA,
            "specialty_lanes": list(self.specialty_lanes),
            "per_specialty_limit": self.per_specialty_limit,
            "timeout_seconds": self.timeout_seconds,
            "request_limit": self.request_limit,
            "record_limit": self.record_limit,
            "response_byte_limit": self.response_byte_limit,
            "total_response_byte_limit": self.total_response_byte_limit,
            "bundle_byte_limit": self.bundle_byte_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "query_set_digest": self.query_set_digest,
            "ncbi_registration_configured": bool(registration),
            "ncbi_registration_digest": self.ncbi_registration_digest,
            "scope": _scope(),
            "execution": _CONFIG_EXECUTION,
            "retention": _CONFIG_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        payload = self._payload()
        return {**payload, "config_digest": content_digest(payload)}

    @classmethod
    def from_dict(
        cls,
        value: Mapping[str, Any],
        *,
        ncbi_tool: str | None = None,
        ncbi_email: str | None = None,
    ) -> "ReviewedPubMedRetrievalConfig":
        """Rehydrate a config, supplying the contact pair again when its artifact binds one."""

        raw = _exact_dict(value, _CONFIG_KEYS, "reviewed PubMed retrieval config")
        if raw["schema"] != REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval config schema is unsupported"
            )
        result = cls(
            specialty_lanes=_lanes(raw["specialty_lanes"]),
            per_specialty_limit=raw["per_specialty_limit"],
            timeout_seconds=raw["timeout_seconds"],
            response_byte_limit=raw["response_byte_limit"],
            total_response_byte_limit=raw["total_response_byte_limit"],
            bundle_byte_limit=raw["bundle_byte_limit"],
            transport_id=raw["transport_id"],
            transport_version=raw["transport_version"],
            transport_config_digest=raw["transport_config_digest"],
            query_set_digest=raw["query_set_digest"],
            ncbi_tool=ncbi_tool,
            ncbi_email=ncbi_email,
            ncbi_registration_digest=raw["ncbi_registration_digest"],
        )
        if canonical_json(raw) != canonical_json(result.to_dict()):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval config is not canonical"
            )
        return result


@dataclass(frozen=True, slots=True)
class ReviewedPubMedRetrievalPlan:
    """Metadata-only review artifact.  Possessing it does not authorize retrieval."""

    config_digest: str
    specialty_lanes: tuple[str, ...]
    per_specialty_limit: int
    request_limit: int
    record_limit: int
    response_byte_limit: int
    total_response_byte_limit: int
    bundle_byte_limit: int
    transport_id: str
    transport_version: str
    transport_config_digest: str
    query_set_digest: str
    ncbi_registration_configured: bool
    ncbi_registration_digest: str
    plan_digest: str

    def __post_init__(self) -> None:
        _digest(self.config_digest, "plan config_digest")
        normalized_lanes = _lanes(self.specialty_lanes, "plan specialty_lanes")
        per_lane = _integer(
            self.per_specialty_limit,
            "plan per_specialty_limit",
            1,
            MAX_PER_SPECIALTY_LIMIT,
        )
        request_limit = _integer(
            self.request_limit, "plan request_limit", 3, MAX_REVIEWED_PUBMED_REQUESTS
        )
        record_limit = _integer(
            self.record_limit, "plan record_limit", 1, MAX_REVIEWED_PUBMED_RECORDS
        )
        _integer(
            self.response_byte_limit,
            "plan response_byte_limit",
            256,
            MAX_RESPONSE_BYTES,
        )
        _integer(
            self.total_response_byte_limit,
            "plan total_response_byte_limit",
            self.response_byte_limit,
            MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES,
        )
        _integer(
            self.bundle_byte_limit,
            "plan bundle_byte_limit",
            1_024,
            MAX_REVIEWED_PUBMED_BUNDLE_BYTES,
        )
        if (
            request_limit != len(normalized_lanes) * 3
            or record_limit != len(normalized_lanes) * per_lane
        ):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed plan bounds do not match its selected lanes"
            )
        object.__setattr__(self, "specialty_lanes", normalized_lanes)
        object.__setattr__(
            self, "transport_id", _identifier(self.transport_id, "plan transport_id")
        )
        object.__setattr__(
            self,
            "transport_version",
            _identifier(self.transport_version, "plan transport_version"),
        )
        _digest(self.transport_config_digest, "plan transport_config_digest")
        if _digest(self.query_set_digest, "plan query_set_digest") != _query_set_digest(
            _query_terms_for(normalized_lanes)
        ):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed plan query set does not match its selected lanes"
            )
        if type(self.ncbi_registration_configured) is not bool:
            raise ReviewedPubMedRetrievalError(
                "plan ncbi_registration_configured must be a boolean"
            )
        registration_digest = _digest(
            self.ncbi_registration_digest,
            "plan ncbi_registration_digest",
        )
        if (
            not self.ncbi_registration_configured
            and registration_digest != _UNCONFIGURED_NCBI_REGISTRATION_DIGEST
        ):
            raise ReviewedPubMedRetrievalError(
                "unconfigured reviewed PubMed plan has an invalid NCBI registration digest"
            )
        _digest(self.plan_digest, "plan plan_digest")
        if content_digest(self._payload()) != self.plan_digest:
            raise ReviewedPubMedRetrievalError("reviewed PubMed plan digest is invalid")
        _bounded_artifact(self.to_dict(), "reviewed PubMed retrieval plan")

    def _payload(self) -> dict[str, Any]:
        return _plan_payload(
            {
                "config_digest": self.config_digest,
                "specialty_lanes": self.specialty_lanes,
                "per_specialty_limit": self.per_specialty_limit,
                "request_limit": self.request_limit,
                "record_limit": self.record_limit,
                "response_byte_limit": self.response_byte_limit,
                "total_response_byte_limit": self.total_response_byte_limit,
                "bundle_byte_limit": self.bundle_byte_limit,
                "transport_id": self.transport_id,
                "transport_version": self.transport_version,
                "transport_config_digest": self.transport_config_digest,
                "query_set_digest": self.query_set_digest,
                "ncbi_registration_configured": self.ncbi_registration_configured,
                "ncbi_registration_digest": self.ncbi_registration_digest,
            }
        )

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "plan_digest": self.plan_digest}

    @classmethod
    def from_config(
        cls, config: ReviewedPubMedRetrievalConfig
    ) -> "ReviewedPubMedRetrievalPlan":
        if type(config) is not ReviewedPubMedRetrievalConfig:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed plan requires an exact config"
            )
        values = {
            "config_digest": config.config_digest,
            "specialty_lanes": config.specialty_lanes,
            "per_specialty_limit": config.per_specialty_limit,
            "request_limit": config.request_limit,
            "record_limit": config.record_limit,
            "response_byte_limit": config.response_byte_limit,
            "total_response_byte_limit": config.total_response_byte_limit,
            "bundle_byte_limit": config.bundle_byte_limit,
            "transport_id": config.transport_id,
            "transport_version": config.transport_version,
            "transport_config_digest": config.transport_config_digest,
            "query_set_digest": config.query_set_digest,
            "ncbi_registration_configured": config.ncbi_registration_configured,
            "ncbi_registration_digest": config.ncbi_registration_digest,
        }
        return cls(**values, plan_digest=content_digest(_plan_payload(values)))

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "ReviewedPubMedRetrievalPlan":
        raw = _exact_dict(value, _PLAN_KEYS, "reviewed PubMed retrieval plan")
        if raw["schema"] != REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval plan schema is unsupported"
            )
        result = cls(
            config_digest=raw["config_digest"],
            specialty_lanes=_lanes(raw["specialty_lanes"], "plan specialty_lanes"),
            per_specialty_limit=raw["per_specialty_limit"],
            request_limit=raw["request_limit"],
            record_limit=raw["record_limit"],
            response_byte_limit=raw["response_byte_limit"],
            total_response_byte_limit=raw["total_response_byte_limit"],
            bundle_byte_limit=raw["bundle_byte_limit"],
            transport_id=raw["transport_id"],
            transport_version=raw["transport_version"],
            transport_config_digest=raw["transport_config_digest"],
            query_set_digest=raw["query_set_digest"],
            ncbi_registration_configured=raw["ncbi_registration_configured"],
            ncbi_registration_digest=raw["ncbi_registration_digest"],
            plan_digest=raw["plan_digest"],
        )
        if canonical_json(raw) != canonical_json(result.to_dict()):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval plan is not canonical"
            )
        return result


@dataclass(frozen=True, slots=True)
class ReviewedPubMedSourceReceipt:
    specialty_lane: str
    source_id: str
    content_digest: str
    record_count: int

    def __post_init__(self) -> None:
        lane = _lanes((self.specialty_lane,), "source receipt specialty_lane")[0]
        if self.source_id != f"pubmed_{lane}":
            raise ReviewedPubMedRetrievalError(
                "source receipt ID does not match its specialty lane"
            )
        _digest(self.content_digest, "source receipt content_digest")
        _integer(
            self.record_count, "source receipt record_count", 1, MAX_PER_SPECIALTY_LIMIT
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA,
            "specialty_lane": self.specialty_lane,
            "source_id": self.source_id,
            "content_digest": self.content_digest,
            "record_count": self.record_count,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "ReviewedPubMedSourceReceipt":
        raw = _exact_dict(value, _SOURCE_RECEIPT_KEYS, "reviewed PubMed source receipt")
        if raw["schema"] != REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed source receipt schema is unsupported"
            )
        result = cls(
            raw["specialty_lane"],
            raw["source_id"],
            raw["content_digest"],
            raw["record_count"],
        )
        if canonical_json(raw) != canonical_json(result.to_dict()):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed source receipt is not canonical"
            )
        return result


@dataclass(frozen=True, slots=True)
class ReviewedPubMedRetrievalReceipt:
    """Metadata-only successful retrieval receipt; source values are deliberately absent."""

    plan_digest: str
    config_digest: str
    specialty_lanes: tuple[str, ...]
    transport_id: str
    transport_version: str
    transport_config_digest: str
    query_set_digest: str
    ncbi_registration_configured: bool
    ncbi_registration_digest: str
    generated_at: str
    bundle_schema: str
    bundle_digest: str
    source_set_digest: str
    sources: tuple[ReviewedPubMedSourceReceipt, ...]
    source_count: int
    record_count: int
    abstract_count: int
    request_count: int
    response_bytes: int
    receipt_digest: str

    def __post_init__(self) -> None:
        for name, value in (
            ("plan_digest", self.plan_digest),
            ("config_digest", self.config_digest),
            ("transport_config_digest", self.transport_config_digest),
            ("query_set_digest", self.query_set_digest),
            ("ncbi_registration_digest", self.ncbi_registration_digest),
            ("bundle_digest", self.bundle_digest),
            ("source_set_digest", self.source_set_digest),
            ("receipt_digest", self.receipt_digest),
        ):
            _digest(value, f"retrieval receipt {name}")
        lanes = _lanes(self.specialty_lanes, "retrieval receipt specialty_lanes")
        object.__setattr__(self, "specialty_lanes", lanes)
        object.__setattr__(
            self,
            "transport_id",
            _identifier(self.transport_id, "retrieval receipt transport_id"),
        )
        object.__setattr__(
            self,
            "transport_version",
            _identifier(self.transport_version, "retrieval receipt transport_version"),
        )
        if self.query_set_digest != _query_set_digest(_query_terms_for(lanes)):
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt query set does not match its selected lanes"
            )
        if type(self.ncbi_registration_configured) is not bool:
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt ncbi_registration_configured must be a boolean"
            )
        if (
            not self.ncbi_registration_configured
            and self.ncbi_registration_digest != _UNCONFIGURED_NCBI_REGISTRATION_DIGEST
        ):
            raise ReviewedPubMedRetrievalError(
                "unconfigured retrieval receipt has an invalid NCBI registration digest"
            )
        _timestamp(self.generated_at, "retrieval receipt generated_at")
        if self.bundle_schema != PUBLIC_LITERATURE_SCHEMA_VERSION:
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt bundle schema is unsupported"
            )
        if (
            not isinstance(self.sources, tuple)
            or any(
                type(source) is not ReviewedPubMedSourceReceipt
                for source in self.sources
            )
            or tuple(source.specialty_lane for source in self.sources) != lanes
        ):
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt sources do not match its selected lanes"
            )
        source_count = _integer(
            self.source_count, "retrieval receipt source_count", 1, MAX_PUBMED_LANES
        )
        record_count = _integer(
            self.record_count,
            "retrieval receipt record_count",
            1,
            MAX_REVIEWED_PUBMED_RECORDS,
        )
        _integer(
            self.abstract_count, "retrieval receipt abstract_count", 0, record_count
        )
        request_count = _integer(
            self.request_count,
            "retrieval receipt request_count",
            3,
            MAX_REVIEWED_PUBMED_REQUESTS,
        )
        _integer(
            self.response_bytes,
            "retrieval receipt response_bytes",
            1,
            MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES,
        )
        if source_count != len(lanes) or request_count != len(lanes) * 3:
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt counts do not match its lanes"
            )
        if record_count != sum(source.record_count for source in self.sources):
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt record count does not match its sources"
            )
        if (
            content_digest([source.to_dict() for source in self.sources])
            != self.source_set_digest
        ):
            raise ReviewedPubMedRetrievalError(
                "retrieval receipt source-set digest is invalid"
            )
        if content_digest(self._payload()) != self.receipt_digest:
            raise ReviewedPubMedRetrievalError("retrieval receipt digest is invalid")
        _bounded_artifact(self.to_dict(), "reviewed PubMed retrieval receipt")

    def _payload(self) -> dict[str, Any]:
        return _receipt_payload(
            {
                "plan_digest": self.plan_digest,
                "config_digest": self.config_digest,
                "specialty_lanes": self.specialty_lanes,
                "transport_id": self.transport_id,
                "transport_version": self.transport_version,
                "transport_config_digest": self.transport_config_digest,
                "query_set_digest": self.query_set_digest,
                "ncbi_registration_configured": self.ncbi_registration_configured,
                "ncbi_registration_digest": self.ncbi_registration_digest,
                "generated_at": self.generated_at,
                "bundle_schema": self.bundle_schema,
                "bundle_digest": self.bundle_digest,
                "source_set_digest": self.source_set_digest,
                "sources": self.sources,
                "source_count": self.source_count,
                "record_count": self.record_count,
                "abstract_count": self.abstract_count,
                "request_count": self.request_count,
                "response_bytes": self.response_bytes,
            }
        )

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "receipt_digest": self.receipt_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "ReviewedPubMedRetrievalReceipt":
        raw = _exact_dict(value, _RECEIPT_KEYS, "reviewed PubMed retrieval receipt")
        if raw["schema"] != REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval receipt schema is unsupported"
            )
        if (
            raw["synthetic_data"] is not False
            or raw["human_review_required"] is not True
        ):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval receipt posture is invalid"
            )
        if type(raw["sources"]) is not list:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval receipt sources must be a list"
            )
        result = cls(
            plan_digest=raw["plan_digest"],
            config_digest=raw["config_digest"],
            specialty_lanes=_lanes(
                raw["specialty_lanes"], "retrieval receipt specialty_lanes"
            ),
            transport_id=raw["transport_id"],
            transport_version=raw["transport_version"],
            transport_config_digest=raw["transport_config_digest"],
            query_set_digest=raw["query_set_digest"],
            ncbi_registration_configured=raw["ncbi_registration_configured"],
            ncbi_registration_digest=raw["ncbi_registration_digest"],
            generated_at=raw["generated_at"],
            bundle_schema=raw["bundle_schema"],
            bundle_digest=raw["bundle_digest"],
            source_set_digest=raw["source_set_digest"],
            sources=tuple(
                ReviewedPubMedSourceReceipt.from_dict(source)
                for source in raw["sources"]
            ),
            source_count=raw["source_count"],
            record_count=raw["record_count"],
            abstract_count=raw["abstract_count"],
            request_count=raw["request_count"],
            response_bytes=raw["response_bytes"],
            receipt_digest=raw["receipt_digest"],
        )
        if canonical_json(raw) != canonical_json(result.to_dict()):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval receipt is not canonical"
            )
        return result


@dataclass(frozen=True, slots=True, repr=False)
class ReviewedPubMedRetrievalResult:
    """A transient bundle plus a safe receipt; the raw bundle is omitted from ``repr``."""

    _bundle_json: str = field(repr=False)
    receipt: ReviewedPubMedRetrievalReceipt

    def __post_init__(self) -> None:
        if (
            type(self._bundle_json) is not str
            or type(self.receipt) is not ReviewedPubMedRetrievalReceipt
        ):
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed retrieval result is malformed"
            )
        try:
            bundle = json.loads(self._bundle_json)
            observed_digest = bundle_digest(bundle)
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            raise ReviewedPubMedRetrievalError(
                "transient PubMed bundle is malformed"
            ) from error
        if observed_digest != self.receipt.bundle_digest:
            raise ReviewedPubMedRetrievalError(
                "transient PubMed bundle does not match its receipt"
            )

    @property
    def bundle(self) -> dict[str, Any]:
        """Return a detached caller-owned copy of the transient bundle."""

        value = json.loads(self._bundle_json)
        if type(value) is not dict:
            raise ReviewedPubMedRetrievalError("transient PubMed bundle is malformed")
        return value

    @property
    def report(self) -> ReviewedPubMedRetrievalReceipt:
        """Alias the metadata-only receipt for report-oriented callers."""

        return self.receipt

    def to_transient_dict(self) -> dict[str, Any]:
        if len(self.receipt.specialty_lanes) != 1:
            raise ReviewedPubMedRetrievalError(
                "generic adapter values require a single-lane reviewed plan"
            )
        return {
            "schema": REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA,
            "lane": self.receipt.specialty_lanes[0],
            "bundle": self.bundle,
            "receipt": self.receipt.to_dict(),
            "retention": _TRANSIENT_RETENTION,
        }

    def __repr__(self) -> str:
        return f"ReviewedPubMedRetrievalResult(receipt={self.receipt!r}, bundle=<transient>)"


def _callable_code(value: Callable[..., Any]) -> Any:
    function = value.__func__ if isinstance(value, MethodType) else value
    return getattr(function, "__code__", None)


_PUBLIC_REFRESH_CALLABLE_NAMES = (
    "refresh_neurosurgical_public_literature",
    "validate_public_literature_bundle",
    "bundle_digest",
    "_pubmed_url",
    "_ncbi_registration_parameters",
    "_json_response",
    "_xml_response",
    "_bounded_abstract",
    "_text",
    "_normalise_date",
    "_source_hash",
    "_summary",
    "_rust_json_bytes",
    "_rust_record_projection",
)
_PUBLIC_REFRESH_CONSTANT_NAMES = (
    "PUBLIC_LITERATURE_SCHEMA_VERSION",
    "PUBMED_AUTHORITY",
    "PUBMED_EUTILS_BASE",
    "MAX_PUBMED_LANES",
    "MAX_PER_SPECIALTY_LIMIT",
    "MAX_TOTAL_RECORDS",
    "MAX_RESPONSE_BYTES",
    "MAX_ABSTRACT_BYTES",
    "MAX_TAGS",
    "MAX_TEXT_BYTES",
)


def _capture_public_refresh_surface() -> tuple[tuple[Any, ...], tuple[Any, ...]]:
    namespace = vars(_public_literature_module)
    callables: list[tuple[Any, ...]] = []
    for name in _PUBLIC_REFRESH_CALLABLE_NAMES:
        value = namespace.get(name)
        if not callable(value):
            raise ReviewedPubMedRetrievalError(
                "public-literature refresh surface is incomplete"
            )
        callables.append((name, value, _callable_code(value)))
    constants = tuple(
        (name, type(namespace.get(name)), namespace.get(name))
        for name in _PUBLIC_REFRESH_CONSTANT_NAMES
    )
    return tuple(callables), constants


def _assert_public_refresh_surface(
    anchor: tuple[tuple[Any, ...], tuple[Any, ...]],
) -> None:
    namespace = vars(_public_literature_module)
    callable_anchor, constant_anchor = anchor
    for name, expected, expected_code in callable_anchor:
        current = namespace.get(name)
        if current is not expected or _callable_code(current) is not expected_code:
            raise ReviewedPubMedRetrievalError(
                "public-literature refresh implementation changed after review"
            )
    for name, expected_type, expected in constant_anchor:
        current = namespace.get(name)
        if type(current) is not expected_type or current != expected:
            raise ReviewedPubMedRetrievalError(
                "public-literature refresh constants changed after review"
            )


class _RejectPubMedRedirects(HTTPRedirectHandler):
    def redirect_request(
        self, request: Request, fp: Any, code: int, msg: str, headers: Any, newurl: str
    ):
        raise ReviewedPubMedRetrievalError(
            "reviewed PubMed transport refuses redirects"
        )


def _reviewed_default_fetch(url: str, *, timeout: float, max_bytes: int) -> bytes:
    parsed = urlsplit(url)
    if (
        parsed.scheme != "https"
        or parsed.hostname != REVIEWED_PUBMED_HOST
        or parsed.netloc != REVIEWED_PUBMED_HOST
        or parsed.path not in _scope()["paths"]
        or parsed.fragment
    ):
        raise ReviewedPubMedRetrievalError(
            "reviewed PubMed transport received an out-of-scope URL"
        )
    request = Request(
        url,
        headers={
            "Accept": "application/json, application/xml",
            "User-Agent": "aurora-agent/0.1",
        },
        method="GET",
    )
    try:
        with build_opener(_RejectPubMedRedirects()).open(
            request, timeout=timeout
        ) as response:  # nosec B310 - exact HTTPS scope checked above
            body = response.read(max_bytes + 1)
    except ReviewedPubMedRetrievalError:
        raise
    except OSError as error:
        raise ReviewedPubMedRetrievalError("reviewed PubMed request failed") from error
    if len(body) > max_bytes:
        raise ReviewedPubMedRetrievalError(
            "PubMed response exceeds the reviewed per-response byte limit"
        )
    return body


_CANONICAL_PUBLIC_REFRESH_SURFACE = _capture_public_refresh_surface()


def _validate_json_tree(value: Any, *, name: str, byte_limit: int) -> None:
    nodes = 0
    observed_scalar_bytes = 0
    stack: list[tuple[Any, int]] = [(value, 0)]
    while stack:
        item, depth = stack.pop()
        nodes += 1
        if nodes > MAX_REVIEWED_PUBMED_RESPONSE_NODES:
            raise ReviewedPubMedRetrievalError(f"{name} contains too many nodes")
        if depth > MAX_REVIEWED_PUBMED_RESPONSE_DEPTH:
            raise ReviewedPubMedRetrievalError(f"{name} is too deeply nested")
        if item is None or type(item) is bool:
            continue
        if type(item) is str:
            observed_scalar_bytes += len(item.encode("utf-8"))
            if observed_scalar_bytes > byte_limit:
                raise ReviewedPubMedRetrievalError(
                    f"{name} exceeds its scalar byte bound"
                )
            continue
        if type(item) is int:
            if not -(2**63) <= item <= 2**64 - 1:
                raise ReviewedPubMedRetrievalError(
                    f"{name} contains an out-of-range integer"
                )
            continue
        if type(item) is float:
            if not math.isfinite(item):
                raise ReviewedPubMedRetrievalError(
                    f"{name} contains a non-finite number"
                )
            continue
        if type(item) is list:
            if len(item) > MAX_REVIEWED_PUBMED_RESPONSE_NODES:
                raise ReviewedPubMedRetrievalError(f"{name} contains an oversized list")
            stack.extend((child, depth + 1) for child in item)
            continue
        if type(item) is dict:
            if len(item) > MAX_REVIEWED_PUBMED_RESPONSE_NODES:
                raise ReviewedPubMedRetrievalError(
                    f"{name} contains an oversized object"
                )
            for key, child in item.items():
                if (
                    type(key) is not str
                    or len(key.encode("utf-8")) > 16_000
                    or "\x00" in key
                ):
                    raise ReviewedPubMedRetrievalError(
                        f"{name} contains an invalid object key"
                    )
                observed_scalar_bytes += len(key.encode("utf-8"))
                if observed_scalar_bytes > byte_limit:
                    raise ReviewedPubMedRetrievalError(
                        f"{name} exceeds its scalar byte bound"
                    )
                stack.append((child, depth + 1))
            continue
        raise ReviewedPubMedRetrievalError(f"{name} contains an unsupported value type")


def _validate_xml_tree(raw: bytes, *, name: str) -> bytes:
    lowered = raw.lower()
    if b"<!entity" in lowered:
        raise ReviewedPubMedRetrievalError(
            f"{name} contains a forbidden document declaration"
        )
    # NCBI's live PubMed EFetch response includes one versioned, external NLM declaration.
    # ElementTree does not need that DTD for this metadata projection, so accept only the exact
    # allow-listed declaration shape and strip it before parsing. Internal subsets, alternate
    # hosts/paths, duplicate declarations, and arbitrary entity definitions remain forbidden.
    if b"<!doctype" in lowered:
        matches = tuple(_PUBMED_DOCTYPE_RE.finditer(raw))
        if len(matches) != 1 or lowered.count(b"<!doctype") != 1:
            raise ReviewedPubMedRetrievalError(
                f"{name} contains a forbidden document declaration"
            )
        match = matches[0]
        prefix = raw[: match.start()]
        if len(prefix) > 256 or _PUBMED_XML_PREFIX_RE.fullmatch(prefix) is None:
            raise ReviewedPubMedRetrievalError(
                f"{name} contains a misplaced document declaration"
            )
        raw = raw[: match.start()] + raw[match.end() :]
    try:
        root = ET.fromstring(raw)
    except (ET.ParseError, ValueError) as error:
        raise ReviewedPubMedRetrievalError(f"{name} is malformed XML") from error
    nodes = 0
    stack: list[tuple[ET.Element, int]] = [(root, 0)]
    while stack:
        node, depth = stack.pop()
        nodes += 1
        if nodes > MAX_REVIEWED_PUBMED_RESPONSE_NODES:
            raise ReviewedPubMedRetrievalError(f"{name} contains too many XML nodes")
        if depth > MAX_REVIEWED_PUBMED_RESPONSE_DEPTH:
            raise ReviewedPubMedRetrievalError(f"{name} XML is too deeply nested")
        if len(node.attrib) > 64:
            raise ReviewedPubMedRetrievalError(
                f"{name} XML node contains too many attributes"
            )
        stack.extend((child, depth + 1) for child in list(node))
    return raw


def _bounded_response(value: Any, *, endpoint: str, byte_limit: int) -> tuple[Any, int]:
    if type(value) is dict:
        if endpoint == "efetch.fcgi":
            raise ReviewedPubMedRetrievalError(
                "PubMed efetch response must be XML bytes or text"
            )
        _validate_json_tree(
            value, name=f"PubMed {endpoint} response", byte_limit=byte_limit
        )
        try:
            encoded = canonical_json(value).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ReviewedPubMedRetrievalError(
                f"PubMed {endpoint} response is not canonical JSON"
            ) from error
        normalized: Any = json.loads(encoded)
    elif type(value) in {bytes, str}:
        encoded = value if type(value) is bytes else value.encode("utf-8")
        normalized = value
        if len(encoded) > byte_limit:
            raise ReviewedPubMedRetrievalError(
                "PubMed response exceeds the reviewed per-response byte limit"
            )
        if endpoint == "efetch.fcgi":
            # Hand the base parser only the already-validated document, with the allow-listed
            # external NLM DTD declaration removed.  Validation must not parse a sanitized copy
            # and then accidentally return the original declaration-bearing value downstream.
            normalized = _validate_xml_tree(encoded, name="PubMed efetch response")
        else:
            try:
                decoded = json.loads(encoded.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ReviewedPubMedRetrievalError(
                    f"PubMed {endpoint} response is malformed JSON"
                ) from error
            _validate_json_tree(
                decoded, name=f"PubMed {endpoint} response", byte_limit=byte_limit
            )
    else:
        raise ReviewedPubMedRetrievalError(
            "PubMed transport returned an unsupported response type"
        )
    if len(encoded) > byte_limit:
        raise ReviewedPubMedRetrievalError(
            "PubMed response exceeds the reviewed per-response byte limit"
        )
    return normalized, len(encoded)


def _request_parameters(
    url: str,
    *,
    lane: str,
    expected_term: str,
    endpoint: str,
    per_specialty_limit: int,
    ncbi_registration: tuple[tuple[str, str], ...],
) -> dict[str, str]:
    if type(url) is not str or len(url.encode("utf-8")) > 64_000:
        raise ReviewedPubMedRetrievalError(
            "PubMed request URL is outside its byte bound"
        )
    parsed = urlsplit(url)
    try:
        port = parsed.port
    except ValueError as error:
        raise ReviewedPubMedRetrievalError(
            "PubMed request URL has an invalid port"
        ) from error
    expected_path = f"/entrez/eutils/{endpoint}"
    if (
        parsed.scheme != "https"
        or parsed.hostname != REVIEWED_PUBMED_HOST
        or parsed.netloc != REVIEWED_PUBMED_HOST
        or parsed.username is not None
        or parsed.password is not None
        or port not in {None, 443}
        or parsed.path != expected_path
        or parsed.fragment
    ):
        raise ReviewedPubMedRetrievalError(
            "PubMed request escaped the reviewed E-utilities scope"
        )
    try:
        pairs = parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True)
    except ValueError as error:
        raise ReviewedPubMedRetrievalError(
            "PubMed request query is malformed"
        ) from error
    if len({key for key, _ in pairs}) != len(pairs):
        raise ReviewedPubMedRetrievalError(
            "PubMed request contains duplicate parameters"
        )
    parameters = dict(pairs)
    registration_parameters = dict(ncbi_registration)
    if endpoint == "esearch.fcgi":
        expected = {
            "db": "pubmed",
            "term": expected_term,
            "retmax": str(per_specialty_limit),
            "retmode": "json",
            "sort": "pub_date",
            **registration_parameters,
        }
        if parameters != expected:
            raise ReviewedPubMedRetrievalError(
                "PubMed search request differs from the reviewed specialty lane"
            )
    elif endpoint == "esummary.fcgi":
        expected_keys = {"db", "id", "retmode", *registration_parameters}
        if (
            set(parameters) != expected_keys
            or parameters["db"] != "pubmed"
            or parameters["retmode"] != "json"
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed summary request differs from its reviewed scope"
            )
    else:
        expected_keys = {"db", "id", "rettype", "retmode", *registration_parameters}
        if (
            set(parameters) != expected_keys
            or parameters["db"] != "pubmed"
            or parameters["rettype"] != "abstract"
            or parameters["retmode"] != "xml"
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed fetch request differs from its reviewed scope"
            )
    if any(parameters.get(name) != value for name, value in ncbi_registration):
        raise ReviewedPubMedRetrievalError(
            "PubMed request NCBI registration differs from the reviewed config"
        )
    if endpoint != "esearch.fcgi":
        identifiers = parameters["id"].split(",")
        if (
            not 1 <= len(identifiers) <= per_specialty_limit
            or len(set(identifiers)) != len(identifiers)
            or any(
                not identifier.isascii()
                or not identifier.isdigit()
                or len(identifier) > 32
                for identifier in identifiers
            )
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed request contains invalid or excessive PMIDs"
            )
    return parameters


class _BoundedReviewedFetch:
    __slots__ = (
        "_fetch",
        "_guard",
        "_lanes",
        "_query_terms",
        "_ncbi_registration",
        "_per_specialty_limit",
        "_response_byte_limit",
        "_total_response_byte_limit",
        "_expected",
        "_summary_ids",
        "request_count",
        "response_bytes",
    )

    def __init__(
        self,
        fetch: PubMedFetcher,
        config: ReviewedPubMedRetrievalConfig,
        query_terms: tuple[tuple[str, str], ...],
        guard: Callable[[], None],
    ) -> None:
        self._fetch = fetch
        self._guard = guard
        self._lanes = config.specialty_lanes
        self._query_terms = query_terms
        self._ncbi_registration = _ncbi_registration(
            config.ncbi_tool, config.ncbi_email
        )
        self._per_specialty_limit = config.per_specialty_limit
        self._response_byte_limit = config.response_byte_limit
        self._total_response_byte_limit = config.total_response_byte_limit
        self._expected = tuple(
            (lane, endpoint)
            for lane in self._lanes
            for endpoint in REVIEWED_PUBMED_ENDPOINTS
        )
        self._summary_ids: dict[str, str] = {}
        self.request_count = 0
        self.response_bytes = 0

    def __call__(self, url: str) -> Any:
        self._guard()
        if self.request_count >= len(self._expected):
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval exceeded its reviewed request count"
            )
        lane, endpoint = self._expected[self.request_count]
        expected_term = dict(self._query_terms)[lane]
        parameters = _request_parameters(
            url,
            lane=lane,
            expected_term=expected_term,
            endpoint=endpoint,
            per_specialty_limit=self._per_specialty_limit,
            ncbi_registration=self._ncbi_registration,
        )
        if endpoint == "esummary.fcgi":
            self._summary_ids[lane] = parameters["id"]
        elif endpoint == "efetch.fcgi" and parameters["id"] != self._summary_ids.get(
            lane
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed summary and fetch PMID sets differ"
            )
        self.request_count += 1
        response = self._fetch(url)
        normalized, response_bytes = _bounded_response(
            response,
            endpoint=endpoint,
            byte_limit=self._response_byte_limit,
        )
        if self.response_bytes + response_bytes > self._total_response_byte_limit:
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval exceeds its reviewed total response byte limit"
            )
        self.response_bytes += response_bytes
        return normalized

    def assert_complete(self) -> None:
        if self.request_count != len(self._expected):
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval did not complete its reviewed request sequence"
            )


def _strict_bundle(
    bundle: Any,
    *,
    config: ReviewedPubMedRetrievalConfig,
    query_terms: tuple[tuple[str, str], ...],
) -> tuple[dict[str, Any], int]:
    raw = _exact_dict(bundle, _BUNDLE_KEYS, "transient PubMed bundle")
    if type(raw["sources"]) is not list or type(raw["records"]) is not list:
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle collections must be lists"
        )
    if len(raw["sources"]) != len(config.specialty_lanes):
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle source count differs from the reviewed plan"
        )
    if not 1 <= len(raw["records"]) <= config.record_limit:
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle record count exceeds the reviewed plan"
        )
    expected_source_ids = tuple(f"pubmed_{lane}" for lane in config.specialty_lanes)
    observed_source_ids: list[str] = []
    for index, source in enumerate(raw["sources"]):
        source_raw = _exact_dict(
            source, _BUNDLE_SOURCE_KEYS, f"transient PubMed source {index}"
        )
        expected_id = expected_source_ids[index]
        if (
            source_raw["source_id"] != expected_id
            or source_raw["authority"] != PUBMED_AUTHORITY
        ):
            raise ReviewedPubMedRetrievalError(
                "transient PubMed source differs from the reviewed lane order"
            )
        _request_parameters(
            source_raw["uri"],
            lane=config.specialty_lanes[index],
            expected_term=dict(query_terms)[config.specialty_lanes[index]],
            endpoint="esearch.fcgi",
            per_specialty_limit=config.per_specialty_limit,
            ncbi_registration=(),
        )
        observed_source_ids.append(expected_id)
    lane_counts = {lane: 0 for lane in config.specialty_lanes}
    for index, record in enumerate(raw["records"]):
        record_raw = _exact_dict(
            record, _BUNDLE_RECORD_KEYS, f"transient PubMed record {index}"
        )
        lane = record_raw["specialty"]
        if lane not in lane_counts or record_raw["source_id"] != f"pubmed_{lane}":
            raise ReviewedPubMedRetrievalError(
                "transient PubMed record escaped its reviewed specialty lane"
            )
        lane_counts[lane] += 1
        if lane_counts[lane] > config.per_specialty_limit:
            raise ReviewedPubMedRetrievalError(
                "transient PubMed lane exceeds its reviewed record limit"
            )
    if any(count < 1 for count in lane_counts.values()):
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle is missing a reviewed specialty lane"
        )
    validate_public_literature_bundle(raw)
    try:
        encoded = canonical_json(raw).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle is not canonical JSON"
        ) from error
    if len(encoded) > config.bundle_byte_limit:
        raise ReviewedPubMedRetrievalError(
            "transient PubMed bundle exceeds its reviewed byte limit"
        )
    detached = json.loads(encoded)
    return detached, len(encoded)


class ReviewedPubMedRetrievalAdapter:
    """Prepare and execute one immutable reviewed retrieval configuration."""

    __slots__ = (
        "_config",
        "_config_anchor",
        "_config_digest_anchor",
        "_fetch",
        "_fetch_anchor",
        "_fetch_code_anchor",
        "_fetch_dependency_anchor",
        "_fetch_dependency_code_anchor",
        "_refresh",
        "_refresh_anchor",
        "_query_terms_anchor",
        "_ncbi_registration_anchor",
        "_catalogue_anchor",
        "_public_surface_anchor",
    )

    def __init__(
        self,
        config: ReviewedPubMedRetrievalConfig,
        *,
        fetch: PubMedFetcher | None = None,
    ) -> None:
        if type(config) is not ReviewedPubMedRetrievalConfig:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed adapter requires an exact config"
            )
        catalogue = vars(_public_literature_module).get("PUBMED_SPECIALTY_LANES")
        if (
            type(catalogue) is not dict
            or catalogue is not PUBMED_SPECIALTY_LANES
            or tuple(catalogue.items()) != _CANONICAL_SPECIALTY_QUERY_TERMS
        ):
            raise ReviewedPubMedRetrievalError(
                "fixed PubMed specialty query catalogue changed before review"
            )
        query_terms = _query_terms_for(config.specialty_lanes)
        ncbi_registration = _ncbi_registration(config.ncbi_tool, config.ncbi_email)
        if config.query_set_digest != _query_set_digest(query_terms):
            raise ReviewedPubMedRetrievalError(
                "PubMed config query set differs from the fixed catalogue"
            )
        if config.ncbi_registration_digest != _ncbi_registration_digest(
            ncbi_registration
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed config NCBI registration binding is invalid"
            )
        _assert_public_refresh_surface(_CANONICAL_PUBLIC_REFRESH_SURFACE)
        public_surface = _capture_public_refresh_surface()
        if fetch is None:
            if (
                config.transport_id != BUILTIN_PUBMED_TRANSPORT_ID
                or config.transport_version != BUILTIN_PUBMED_TRANSPORT_VERSION
                or config.transport_config_digest
                != BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST
            ):
                raise ReviewedPubMedRetrievalError(
                    "builtin PubMed retrieval requires its exact transport identity"
                )
            last_request = [0.0]
            monotonic = time.monotonic
            sleep = time.sleep
            implementation = _reviewed_default_fetch
            timeout_seconds = config.timeout_seconds
            response_byte_limit = config.response_byte_limit

            def captured_fetch(url: str) -> bytes:
                elapsed = monotonic() - last_request[0]
                if elapsed < 0.34:
                    sleep(0.34 - elapsed)
                body = implementation(
                    url, timeout=timeout_seconds, max_bytes=response_byte_limit
                )
                last_request[0] = monotonic()
                return body

            selected_fetch: PubMedFetcher = captured_fetch
            fetch_dependency: Callable[..., Any] | None = implementation
        else:
            if not callable(fetch):
                raise ReviewedPubMedRetrievalError(
                    "injected PubMed fetch must be callable"
                )
            if (
                config.transport_id == BUILTIN_PUBMED_TRANSPORT_ID
                or config.transport_config_digest
                == BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST
            ):
                raise ReviewedPubMedRetrievalError(
                    "an injected PubMed fetch requires a distinct reviewed transport identity"
                )
            if type(fetch).__call__ is not type.__call__ and not hasattr(
                fetch, "__code__"
            ):
                selected_fetch = (
                    fetch.__call__
                )  # capture the current bound implementation
            else:
                selected_fetch = fetch
            fetch_dependency = None
        object.__setattr__(self, "_config", config)
        object.__setattr__(self, "_config_anchor", config)
        object.__setattr__(self, "_config_digest_anchor", config.config_digest)
        object.__setattr__(self, "_fetch", selected_fetch)
        object.__setattr__(self, "_fetch_anchor", selected_fetch)
        object.__setattr__(self, "_fetch_code_anchor", _callable_code(selected_fetch))
        object.__setattr__(self, "_fetch_dependency_anchor", fetch_dependency)
        object.__setattr__(
            self,
            "_fetch_dependency_code_anchor",
            None if fetch_dependency is None else _callable_code(fetch_dependency),
        )
        object.__setattr__(self, "_refresh", _PUBLIC_REFRESH)
        object.__setattr__(self, "_refresh_anchor", _PUBLIC_REFRESH)
        object.__setattr__(self, "_query_terms_anchor", query_terms)
        object.__setattr__(self, "_ncbi_registration_anchor", ncbi_registration)
        object.__setattr__(self, "_catalogue_anchor", catalogue)
        object.__setattr__(self, "_public_surface_anchor", public_surface)

    @property
    def config(self) -> ReviewedPubMedRetrievalConfig:
        return object.__getattribute__(self, "_config_anchor")

    def _guard(self, reviewed_plan: ReviewedPubMedRetrievalPlan) -> None:
        if type(self) is not ReviewedPubMedRetrievalAdapter:
            raise ReviewedPubMedRetrievalError(
                "reviewed PubMed adapter subclasses are not executable"
            )
        if type(reviewed_plan) is not ReviewedPubMedRetrievalPlan:
            raise ReviewedPubMedRetrievalError(
                "PubMed execution requires an exact reviewed plan"
            )
        ReviewedPubMedRetrievalPlan.from_dict(reviewed_plan.to_dict())
        config = object.__getattribute__(self, "_config")
        config_anchor = object.__getattribute__(self, "_config_anchor")
        fetch = object.__getattribute__(self, "_fetch")
        fetch_anchor = object.__getattribute__(self, "_fetch_anchor")
        fetch_dependency = object.__getattribute__(self, "_fetch_dependency_anchor")
        refresh = object.__getattribute__(self, "_refresh")
        refresh_anchor = object.__getattribute__(self, "_refresh_anchor")
        query_terms = object.__getattribute__(self, "_query_terms_anchor")
        ncbi_registration = object.__getattribute__(self, "_ncbi_registration_anchor")
        catalogue = object.__getattribute__(self, "_catalogue_anchor")
        if (
            type(config) is not ReviewedPubMedRetrievalConfig
            or config is not config_anchor
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval config identity changed after review"
            )
        if config.config_digest != object.__getattribute__(
            self, "_config_digest_anchor"
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval config changed after review"
            )
        if (
            fetch is not fetch_anchor
            or not callable(fetch)
            or _callable_code(fetch)
            is not object.__getattribute__(self, "_fetch_code_anchor")
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed fetch callable changed after review"
            )
        if fetch_dependency is not None and (
            fetch_dependency is not _reviewed_default_fetch
            or _callable_code(fetch_dependency)
            is not object.__getattribute__(self, "_fetch_dependency_code_anchor")
        ):
            raise ReviewedPubMedRetrievalError(
                "builtin PubMed fetch implementation changed after review"
            )
        if refresh is not refresh_anchor or refresh is not _PUBLIC_REFRESH:
            raise ReviewedPubMedRetrievalError(
                "PubMed refresh implementation changed after review"
            )
        if (
            vars(_public_literature_module).get("PUBMED_SPECIALTY_LANES")
            is not catalogue
            or catalogue is not PUBMED_SPECIALTY_LANES
            or type(catalogue) is not dict
            or tuple(catalogue.items()) != _CANONICAL_SPECIALTY_QUERY_TERMS
            or query_terms != _query_terms_for(config.specialty_lanes)
            or _query_set_digest(query_terms) != config.query_set_digest
        ):
            raise ReviewedPubMedRetrievalError(
                "fixed PubMed specialty queries changed after review"
            )
        if (
            _ncbi_registration(config.ncbi_tool, config.ncbi_email) != ncbi_registration
            or _ncbi_registration_digest(ncbi_registration)
            != config.ncbi_registration_digest
        ):
            raise ReviewedPubMedRetrievalError("NCBI registration changed after review")
        _assert_public_refresh_surface(
            object.__getattribute__(self, "_public_surface_anchor")
        )
        current_plan = ReviewedPubMedRetrievalPlan.from_config(config)
        if canonical_json(reviewed_plan.to_dict()) != canonical_json(
            current_plan.to_dict()
        ):
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval plan or config drifted after review"
            )

    def prepare(self) -> ReviewedPubMedRetrievalPlan:
        """Return a deterministic review artifact without invoking the transport."""

        config = object.__getattribute__(self, "_config")
        if type(config) is not ReviewedPubMedRetrievalConfig:
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval config identity changed before review"
            )
        return ReviewedPubMedRetrievalPlan.from_config(config)

    def execute(
        self,
        reviewed_plan: ReviewedPubMedRetrievalPlan,
        *,
        approve_source_dispatch: bool,
        retrieved_at: str | None = None,
    ) -> ReviewedPubMedRetrievalResult:
        """Execute the exact reviewed plan once after a literal ``True`` approval."""

        if approve_source_dispatch is not True:
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval requires explicit literal approval"
            )
        if retrieved_at is not None:
            _timestamp(retrieved_at, "retrieved_at")
        self._guard(reviewed_plan)
        config = object.__getattribute__(self, "_config_anchor")
        fetch = object.__getattribute__(self, "_fetch_anchor")
        refresh = object.__getattribute__(self, "_refresh_anchor")

        def guard() -> None:
            self._guard(reviewed_plan)

        query_terms = object.__getattribute__(self, "_query_terms_anchor")
        bounded_fetch = _BoundedReviewedFetch(fetch, config, query_terms, guard)
        try:
            bundle, base_report = refresh(
                fetch=bounded_fetch,
                per_specialty_limit=config.per_specialty_limit,
                specialty_lanes=config.specialty_lanes,
                retrieved_at=retrieved_at,
                timeout=config.timeout_seconds,
                ncbi_tool=config.ncbi_tool,
                ncbi_email=config.ncbi_email,
            )
        except ReviewedPubMedRetrievalError:
            raise
        except PublicLiteratureRefreshError as error:
            raise ReviewedPubMedRetrievalError(
                "PubMed retrieval failed its reviewed source contract"
            ) from error
        bounded_fetch.assert_complete()
        self._guard(reviewed_plan)
        validated_bundle, _bundle_bytes = _strict_bundle(
            bundle, config=config, query_terms=query_terms
        )
        digest = bundle_digest(validated_bundle)
        if base_report.bundle_digest != digest:
            raise ReviewedPubMedRetrievalError(
                "PubMed parser report does not match the transient bundle"
            )
        sources = tuple(
            ReviewedPubMedSourceReceipt(
                specialty_lane=lane,
                source_id=source["source_id"],
                content_digest=source["content_sha256"],
                record_count=source["record_count"],
            )
            for lane, source in zip(
                config.specialty_lanes, validated_bundle["sources"], strict=True
            )
        )
        source_set_digest = content_digest([source.to_dict() for source in sources])
        receipt_values = {
            "plan_digest": reviewed_plan.plan_digest,
            "config_digest": config.config_digest,
            "specialty_lanes": config.specialty_lanes,
            "transport_id": config.transport_id,
            "transport_version": config.transport_version,
            "transport_config_digest": config.transport_config_digest,
            "query_set_digest": config.query_set_digest,
            "ncbi_registration_configured": config.ncbi_registration_configured,
            "ncbi_registration_digest": config.ncbi_registration_digest,
            "generated_at": validated_bundle["generated_at"],
            "bundle_schema": PUBLIC_LITERATURE_SCHEMA_VERSION,
            "bundle_digest": digest,
            "source_set_digest": source_set_digest,
            "sources": sources,
            "source_count": len(sources),
            "record_count": len(validated_bundle["records"]),
            "abstract_count": sum(
                1
                for record in validated_bundle["records"]
                if record["abstract_text"] is not None
            ),
            "request_count": bounded_fetch.request_count,
            "response_bytes": bounded_fetch.response_bytes,
        }
        receipt = ReviewedPubMedRetrievalReceipt(
            **receipt_values,
            receipt_digest=content_digest(_receipt_payload(receipt_values)),
        )
        return ReviewedPubMedRetrievalResult(canonical_json(validated_bundle), receipt)


def create_reviewed_pubmed_execution_metadata(
    reviewed_plan: ReviewedPubMedRetrievalPlan,
    *,
    approve_source_dispatch: bool,
    retrieved_at: str | None = None,
) -> dict[str, Any]:
    """Create the exact per-call metadata expected by the generic adapter callback."""

    if type(reviewed_plan) is not ReviewedPubMedRetrievalPlan:
        raise ReviewedPubMedRetrievalError(
            "PubMed execution metadata requires an exact reviewed plan"
        )
    ReviewedPubMedRetrievalPlan.from_dict(reviewed_plan.to_dict())
    if approve_source_dispatch is not True:
        raise ReviewedPubMedRetrievalError(
            "PubMed execution metadata requires explicit literal approval"
        )
    if retrieved_at is not None:
        _timestamp(retrieved_at, "retrieved_at")
    return {
        "schema": REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA,
        "reviewed_plan_digest": reviewed_plan.plan_digest,
        "approve_source_dispatch": True,
        "retrieved_at": retrieved_at,
    }


def _validated_execution_metadata(value: Any, plan_digest: str) -> dict[str, Any]:
    raw = _exact_dict(
        value, _EXECUTION_METADATA_KEYS, "reviewed PubMed execution metadata"
    )
    if raw["schema"] != REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA:
        raise ReviewedPubMedRetrievalError(
            "reviewed PubMed execution metadata schema is unsupported"
        )
    if raw["reviewed_plan_digest"] != plan_digest:
        raise ReviewedPubMedRetrievalError(
            "reviewed PubMed execution metadata names a different plan"
        )
    if raw["approve_source_dispatch"] is not True:
        raise ReviewedPubMedRetrievalError(
            "PubMed retrieval requires explicit literal approval"
        )
    if raw["retrieved_at"] is not None:
        _timestamp(raw["retrieved_at"], "retrieved_at")
    _bounded_artifact(raw, "reviewed PubMed execution metadata")
    return raw


def create_reviewed_pubmed_autonomous_evidence_registration(
    adapter: ReviewedPubMedRetrievalAdapter,
    reviewed_plan: ReviewedPubMedRetrievalPlan,
    *,
    specialty_lane: str,
) -> AutonomousEvidenceAdapterRegistration:
    """Bind one single-lane reviewed plan to generic acquire/project callbacks.

    The generic evidence request must use ``source_digest=reviewed_plan.plan_digest`` and put the
    result of :func:`create_reviewed_pubmed_execution_metadata` in its metadata field.  Requiring a
    single-lane plan prevents one generic source request from silently widening into other lanes.
    """

    if type(adapter) is not ReviewedPubMedRetrievalAdapter:
        raise ReviewedPubMedRetrievalError(
            "PubMed registration requires an exact reviewed adapter"
        )
    if type(reviewed_plan) is not ReviewedPubMedRetrievalPlan:
        raise ReviewedPubMedRetrievalError(
            "PubMed registration requires an exact reviewed plan"
        )
    lane = _lanes((specialty_lane,), "registration specialty_lane")[0]
    adapter._guard(reviewed_plan)
    if reviewed_plan.specialty_lanes != (lane,):
        raise ReviewedPubMedRetrievalError(
            "generic PubMed registration requires a single-lane reviewed plan"
        )
    frozen_plan = ReviewedPubMedRetrievalPlan.from_dict(reviewed_plan.to_dict())
    frozen_adapter = adapter
    adapter_id = f"ncbi_pubmed_{lane}_{frozen_plan.plan_digest[:16]}"
    expected_source_id = f"pubmed_{lane}"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        if type(context) is not dict:
            raise ReviewedPubMedRetrievalError(
                "generic PubMed acquire context must be an exact mapping"
            )
        request = context.get("request")
        if type(request) is not dict:
            raise ReviewedPubMedRetrievalError(
                "generic PubMed acquire request is malformed"
            )
        if (
            request.get("source_id") != expected_source_id
            or request.get("source_digest") != frozen_plan.plan_digest
        ):
            raise ReviewedPubMedRetrievalError(
                "generic PubMed request does not match its reviewed source"
            )
        metadata = _validated_execution_metadata(
            request.get("metadata"), frozen_plan.plan_digest
        )
        result = frozen_adapter.execute(
            frozen_plan,
            approve_source_dispatch=metadata["approve_source_dispatch"],
            retrieved_at=metadata["retrieved_at"],
        )
        return result.to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        raw = _exact_dict(
            value, _TRANSIENT_VALUE_KEYS, "generic PubMed transient value"
        )
        if (
            raw["schema"] != REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA
            or raw["lane"] != lane
            or raw["retention"] != _TRANSIENT_RETENTION
        ):
            raise ReviewedPubMedRetrievalError(
                "generic PubMed transient value identity is invalid"
            )
        receipt = ReviewedPubMedRetrievalReceipt.from_dict(raw["receipt"])
        if (
            receipt.plan_digest != frozen_plan.plan_digest
            or receipt.specialty_lanes != (lane,)
        ):
            raise ReviewedPubMedRetrievalError(
                "generic PubMed transient value names a different reviewed plan"
            )
        validated_bundle, _size = _strict_bundle(
            raw["bundle"],
            config=frozen_adapter.config,
            query_terms=object.__getattribute__(frozen_adapter, "_query_terms_anchor"),
        )
        if bundle_digest(validated_bundle) != receipt.bundle_digest:
            raise ReviewedPubMedRetrievalError(
                "generic PubMed transient bundle does not match its receipt"
            )
        if not isinstance(context, Mapping):
            raise ReviewedPubMedRetrievalError(
                "generic PubMed project context is malformed"
            )
        requirement = context.get("requirement")
        label = (
            requirement.get("label")
            if isinstance(requirement, Mapping)
            else getattr(requirement, "label", None)
        )
        if type(label) is not str or not label.strip():
            raise ReviewedPubMedRetrievalError(
                "generic PubMed project context has no requirement label"
            )
        return [
            {
                "label": label,
                "kind": "provenance",
                "status": "observed",
                "value_digest": receipt.bundle_digest,
                "source_digest": receipt.source_set_digest,
                "confidence": None,
                "limitations": list(_LIMITATIONS),
            }
        ]

    return AutonomousEvidenceAdapterRegistration(
        adapter_id=adapter_id,
        version=REVIEWED_PUBMED_ADAPTER_VERSION,
        domains=("biomedical", "neuroscience"),
        capabilities=(
            "evidence",
            "literature",
            "provenance",
            "public_literature_refresh",
        ),
        source_kinds=("pubmed", "public_literature"),
        acquire=acquire,
        project=project,
    )


__all__ = [
    "REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA",
    "REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA",
    "REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA",
    "REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA",
    "REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA",
    "REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA",
    "REVIEWED_PUBMED_QUERY_SET_SCHEMA",
    "REVIEWED_PUBMED_NCBI_REGISTRATION_SCHEMA",
    "REVIEWED_PUBMED_ADAPTER_VERSION",
    "REVIEWED_PUBMED_ENDPOINTS",
    "REVIEWED_PUBMED_HOST",
    "MAX_REVIEWED_PUBMED_REQUESTS",
    "MAX_REVIEWED_PUBMED_RECORDS",
    "MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES",
    "MAX_REVIEWED_PUBMED_BUNDLE_BYTES",
    "MAX_REVIEWED_PUBMED_RESPONSE_DEPTH",
    "MAX_REVIEWED_PUBMED_RESPONSE_NODES",
    "BUILTIN_PUBMED_TRANSPORT_ID",
    "BUILTIN_PUBMED_TRANSPORT_VERSION",
    "BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST",
    "ReviewedPubMedRetrievalError",
    "ReviewedPubMedRetrievalConfig",
    "ReviewedPubMedRetrievalPlan",
    "ReviewedPubMedSourceReceipt",
    "ReviewedPubMedRetrievalReceipt",
    "ReviewedPubMedRetrievalResult",
    "ReviewedPubMedRetrievalAdapter",
    "create_reviewed_pubmed_execution_metadata",
    "create_reviewed_pubmed_autonomous_evidence_registration",
]
