"""Reviewed GDC TP53 mutation-affected case aggregates for two public glioma projects.

This repository-defined public-human research reader follows the public API's documented
analysis aggregate shape. It implements no blueprint claim about molecular mechanisms or the
preclinical glioma engine. A source count is not prevalence, an assay denominator, a diagnosis,
or independent evidence. Project/gene buckets that are absent remain unknown, never zero.
"""
from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy
from dataclasses import dataclass, field
from datetime import datetime, timezone
import http.client
import ipaddress
import json
import math
import queue
import re
import socket
import threading
import time
from typing import Any
from urllib.parse import urlencode, urlsplit

from .authoring import canonical_json, content_digest
from .errors import ArgumentError

REVIEWED_GDC_GENE_MUTATION_GENE_ID = "ENSG00000141510"
REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL = "TP53"
REVIEWED_GDC_GENE_MUTATION_PROJECTS = ("TCGA-GBM", "TCGA-LGG")
REVIEWED_GDC_GENE_MUTATION_HOST = "api.gdc.cancer.gov"
REVIEWED_GDC_GENE_MUTATION_PATH = "/analysis/top_cases_counts_by_genes"
REVIEWED_GDC_GENE_MUTATION_ENDPOINT = "https://" + REVIEWED_GDC_GENE_MUTATION_HOST + REVIEWED_GDC_GENE_MUTATION_PATH
REVIEWED_GDC_GENE_MUTATION_AUTHORITY = "NCI Genomic Data Commons"
REVIEWED_GDC_GENE_MUTATION_CONFIG_SCHEMA = "bioprism-reviewed-gdc-gene-mutation-config/0.1"
REVIEWED_GDC_GENE_MUTATION_PLAN_SCHEMA = "bioprism-reviewed-gdc-gene-mutation-plan/0.1"
REVIEWED_GDC_GENE_MUTATION_BUNDLE_SCHEMA = "bioprism-reviewed-gdc-gene-mutation-bundle/0.1"
REVIEWED_GDC_GENE_MUTATION_RECEIPT_SCHEMA = "bioprism-reviewed-gdc-gene-mutation-receipt/0.1"
REVIEWED_GDC_GENE_MUTATION_TRANSIENT_SCHEMA = "bioprism-reviewed-gdc-gene-mutation-transient/0.1"
MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES = 1_000_000
MAX_REVIEWED_GDC_GENE_MUTATION_TREE_DEPTH = 32
MAX_REVIEWED_GDC_GENE_MUTATION_TREE_NODES = 50_000
MAX_REVIEWED_GDC_GENE_MUTATION_PROJECT_BUCKETS = 128
BUILTIN_GDC_GENE_MUTATION_TRANSPORT_ID = "builtin.nci-gdc-gene-mutation.https"
BUILTIN_GDC_GENE_MUTATION_TRANSPORT_VERSION = "1"
_SOURCE_ID = "gdc_tp53_gene_mutation_counts"
_SCOPE = "fixed_public_glioma_tp53_mutation_case_aggregates"
_RETENTION = "aggregate_counts_only;no_individual_case_sample_patient_or_mutation_records"
_LIMITATIONS = (
    "Source-reported mutation-affected case aggregates are not prevalence, assay values, diagnosis, or treatment evidence.",
    "Only the fixed TP53 Ensembl ID and TCGA-GBM/TCGA-LGG project aggregates are retained; the source query returns other project aggregates that are explicitly omitted.",
    "Missing project or gene buckets are unknown, not zero or negative biological evidence.",
    "No gene-specific assay denominator, Entrez crosswalk, cohort independence, or cross-source overlap is established; counts are not pooled.",
    "The endpoint reports public GDC aggregate observations under its own annotation and release semantics; freshness, ascertainment, applicability, and scientific validity require human review.",
    "Self-computed digests establish internal consistency, not authenticated source truth or scientific support.",
    "No individual records, controlled-access material, clinical actions, laboratory actions, or preclinical-engine routing are requested.",
    "Caller-injected transports own timeout, DNS, redirect, and network policy under their declared identity.",
)
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_PROJECT_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.-]{1,128}$")
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


class ReviewedGdcGeneMutationRetrievalError(ArgumentError):
    """The bounded public aggregate contract failed; no empty result is substituted."""


GdcGeneMutationFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedGdcGeneMutationRetrievalError(message)


def _integer(name: str, value: Any, minimum: int = 0, maximum: int = 1_000_000_000_000) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        _fail(name + " is outside its bounded integer range")
    return value


def _exact(value: Any, keys: set[str], name: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != keys:
        _fail(name + " has an unexpected aggregate shape")
    return value


def _digest(value: Any, name: str) -> str:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        _fail(name + " is not a SHA-256 digest")
    return value


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(name + " must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedGdcGeneMutationRetrievalError(name + " is not a real UTC timestamp") from error
    return value


def _tree(value: Any, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_GDC_GENE_MUTATION_TREE_NODES]
    budget[0] -= 1
    if depth > MAX_REVIEWED_GDC_GENE_MUTATION_TREE_DEPTH or budget[0] < 0:
        _fail("GDC gene mutation response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, item in value.items():
            if not isinstance(key, str):
                _fail("GDC gene mutation response contains a non-text key")
            key.encode("utf-8", "strict")
            _tree(item, depth + 1, budget)
    elif isinstance(value, list):
        for item in value:
            _tree(item, depth + 1, budget)
    elif isinstance(value, str):
        value.encode("utf-8", "strict")
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("GDC gene mutation response contains a non-finite number")
    elif value is not None and type(value) not in (int, float, bool):
        _fail("GDC gene mutation response contains an unsupported value")


def _json_copy(value: Any, maximum: int = MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES) -> Any:
    try:
        _tree(value)
        encoded = canonical_json(value)
        if len(encoded.encode("utf-8")) > maximum:
            _fail("GDC gene mutation artifact exceeds its byte bound")
        return json.loads(encoded)
    except (TypeError, ValueError, RecursionError, UnicodeError) as error:
        if isinstance(error, ReviewedGdcGeneMutationRetrievalError):
            raise
        raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation artifact is not bounded canonical JSON") from error


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, item in pairs:
        if key in result:
            _fail("GDC gene mutation response contains duplicate JSON fields")
        result[key] = item
    return result


def _parse_response(value: Any) -> tuple[Mapping[str, Any], int]:
    try:
        if isinstance(value, Mapping):
            parsed = _json_copy(value)
            encoded = canonical_json(parsed).encode("utf-8")
        elif isinstance(value, (str, bytes)):
            encoded = value.encode("utf-8", "strict") if isinstance(value, str) else value
            if len(encoded) > MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES:
                _fail("GDC gene mutation response exceeds its byte bound")
            parsed = json.loads(encoded.decode("utf-8", "strict"), object_pairs_hook=_pairs, parse_constant=lambda _value: _fail("GDC gene mutation response contains a non-finite number"))
            _tree(parsed)
        else:
            _fail("GDC gene mutation response is not JSON")
        if not isinstance(parsed, Mapping):
            _fail("GDC gene mutation response root is not an aggregate object")
        return parsed, len(encoded)
    except (TypeError, ValueError, UnicodeError, RecursionError) as error:
        if isinstance(error, ReviewedGdcGeneMutationRetrievalError):
            raise
        raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation response is not bounded UTF-8 JSON") from error


def _url() -> str:
    return REVIEWED_GDC_GENE_MUTATION_ENDPOINT + "?" + urlencode({"gene_ids": REVIEWED_GDC_GENE_MUTATION_GENE_ID})


BUILTIN_GDC_GENE_MUTATION_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "http.client.HTTPSConnection", "method": "GET", "url": _url(),
    "credentials": "not_accepted", "redirects": "refused", "dns": "all_public_answers_and_pinned_socket",
    "request_limit": 1, "timeout_ms_max": 30_000, "response_bytes_max": MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES,
})


def _remaining(deadline: float) -> float:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        _fail("GDC gene mutation request exceeded its deadline")
    return remaining


def _validate_public_dns(deadline: float) -> tuple[Any, ...]:
    # DNS can block outside socket timeouts. A bounded daemon lookup cannot dispatch a request.
    result: queue.Queue[Any] = queue.Queue(maxsize=1)
    def resolve() -> None:
        try:
            result.put(socket.getaddrinfo(REVIEWED_GDC_GENE_MUTATION_HOST, 443, type=socket.SOCK_STREAM))
        except Exception as error:
            result.put(error)
    threading.Thread(target=resolve, daemon=True).start()
    try:
        addresses = result.get(timeout=_remaining(deadline))
    except queue.Empty as error:
        raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation DNS lookup exceeded its deadline") from error
    if isinstance(addresses, Exception) or not isinstance(addresses, list) or not 1 <= len(addresses) <= 64:
        _fail("GDC gene mutation DNS lookup failed or exceeded its bound")
    try:
        if any(item[0] not in (socket.AF_INET, socket.AF_INET6) or item[1] != socket.SOCK_STREAM or not ipaddress.ip_address(item[4][0]).is_global or item[4][1] != 443 for item in addresses):
            _fail("GDC gene mutation DNS returned a non-public address")
    except (ValueError, TypeError, IndexError) as error:
        raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation DNS returned a malformed address") from error
    return addresses[0]


class _PinnedHttpsConnection(http.client.HTTPSConnection):
    def __init__(self, address: tuple[Any, ...], deadline: float) -> None:
        super().__init__(REVIEWED_GDC_GENE_MUTATION_HOST, 443, timeout=_remaining(deadline))
        self._checked_address, self._deadline = address, deadline

    def connect(self) -> None:
        family, socktype, protocol, _, sockaddr = self._checked_address
        raw = socket.socket(family, socktype, protocol)
        try:
            raw.settimeout(_remaining(self._deadline))
            raw.connect(sockaddr)
            raw.settimeout(_remaining(self._deadline))
            self.sock = self._context.wrap_socket(raw, server_hostname=self.host)
            self.sock.settimeout(_remaining(self._deadline))
        except BaseException:
            raw.close()
            raise


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    if url != _url():
        _fail("GDC gene mutation transport refused an unpinned URL")
    deadline = time.monotonic() + timeout_ms / 1000
    connection = None
    try:
        connection = _PinnedHttpsConnection(_validate_public_dns(deadline), deadline)
        parts = urlsplit(url)
        connection.request("GET", parts.path + "?" + parts.query, headers={"Accept": "application/json", "Accept-Encoding": "identity", "User-Agent": "aurora-agent-reviewed-gdc-aggregate/0.1"})
        active_socket = connection.sock
        if active_socket is not None:
            active_socket.settimeout(_remaining(deadline))
        response = connection.getresponse()
        if response.status != 200 or response.getheader("Location") is not None:
            _fail("GDC gene mutation endpoint returned an unexpected status or redirect")
        content_type = response.getheader("Content-Type", "").split(";", 1)[0].strip().casefold()
        if content_type != "application/json" or response.getheader("Content-Encoding", "identity").casefold() not in ("identity", ""):
            _fail("GDC gene mutation endpoint returned an unsupported content type or encoding")
        length = response.getheader("Content-Length")
        if length is not None and (not re.fullmatch(r"[0-9]{1,10}", length) or int(length) > MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES):
            _fail("GDC gene mutation response exceeds its declared byte bound")
        body = bytearray()
        while True:
            if active_socket is not None:
                active_socket.settimeout(_remaining(deadline))
            else:
                _remaining(deadline)
            chunk = response.read1(min(65_536, MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
            if len(body) > MAX_REVIEWED_GDC_GENE_MUTATION_RESPONSE_BYTES:
                _fail("GDC gene mutation response exceeds its byte bound")
        _remaining(deadline)
        if length is not None and len(body) != int(length):
            _fail("GDC gene mutation response differs from its declared byte length")
        return bytes(body)
    except (OSError, http.client.HTTPException, ValueError) as error:
        if isinstance(error, ReviewedGdcGeneMutationRetrievalError):
            raise
        raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation aggregate request failed") from error
    finally:
        if connection is not None:
            connection.close()


def _aggregation(value: Any, name: str, maximum: int) -> list[Any]:
    item = _exact(value, {"doc_count_error_upper_bound", "sum_other_doc_count", "buckets"}, name)
    if _integer(name + " count error", item["doc_count_error_upper_bound"]) != 0 or _integer(name + " omitted count", item["sum_other_doc_count"]) != 0:
        _fail(name + " has unresolved aggregation error bounds or omissions")
    rows = item["buckets"]
    if not isinstance(rows, list) or len(rows) > maximum:
        _fail(name + " exceeds its bucket bound")
    return rows


def _project_bucket(value: Any, index: int) -> dict[str, Any]:
    row = _exact(value, {"key", "doc_count", "genes"}, "GDC project bucket")
    project_id = row["key"]
    if not isinstance(project_id, str) or not _PROJECT_IDENTIFIER.fullmatch(project_id):
        _fail("GDC aggregate project identity is invalid")
    project_count = _integer("GDC project doc_count", row["doc_count"])
    genes = _exact(row["genes"], {"doc_count", "my_genes"}, "GDC nested genes")
    nested_count = _integer("GDC nested genes doc_count", genes["doc_count"])
    selected = _exact(genes["my_genes"], {"doc_count", "gene_id"}, "GDC selected genes")
    selected_count = _integer("GDC selected genes doc_count", selected["doc_count"])
    buckets = _aggregation(selected["gene_id"], "GDC gene aggregation", 1)
    count = None
    if buckets:
        gene_bucket = _exact(buckets[0], {"key", "doc_count"}, "GDC gene bucket")
        if gene_bucket["key"] != REVIEWED_GDC_GENE_MUTATION_GENE_ID:
            _fail("GDC gene aggregate identity differs from the reviewed TP53 ID")
        count = _integer("GDC gene mutation case count", gene_bucket["doc_count"])
        if count != selected_count or count > project_count or count > nested_count:
            _fail("GDC gene aggregate count metadata is inconsistent")
    elif selected_count != 0:
        _fail("GDC missing gene bucket contradicts its selected count")
    if selected_count > nested_count:
        _fail("GDC selected gene count exceeds its native nested count")
    return {"project_id": project_id, "state": "observed" if count is not None else "gene_bucket_missing", "case_count_with_mutation": count, "project_doc_count": project_count, "nested_genes_doc_count": nested_count, "selected_genes_doc_count": selected_count, "source_json_pointer": f"/aggregations/projects/buckets/{index}/genes/my_genes/gene_id/buckets/0/doc_count" if count is not None else None}


def _projection(response: Mapping[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    _exact(response, {"took", "timed_out", "_shards", "hits", "aggregations"}, "GDC analysis response")
    took = _integer("GDC analysis took", response["took"], maximum=86_400_000)
    if response["timed_out"] is not False:
        _fail("GDC analysis timed_out must be literal false")
    shards = _exact(response["_shards"], {"total", "successful", "skipped", "failed"}, "GDC shard metadata")
    for key in shards:
        _integer("GDC shard " + key, shards[key], maximum=10_000)
    if shards["total"] < 1 or shards["successful"] != shards["total"] or shards["failed"] != 0 or shards["skipped"] > shards["successful"]:
        _fail("GDC analysis has failed or incomplete shards")
    hits = _exact(response["hits"], {"total", "max_score", "hits"}, "GDC aggregate hits")
    if not isinstance(hits["hits"], list) or hits["hits"] or hits["max_score"] is not None:
        _fail("GDC aggregate response contains individual hits or unexpected scoring")
    total = _exact(hits["total"], {"value", "relation"}, "GDC aggregate hit total")
    total_count = _integer("GDC aggregate hit total", total["value"])
    if total["relation"] != "eq":
        _fail("GDC aggregate hit total is not exact")
    aggregations = _exact(response["aggregations"], {"projects"}, "GDC analysis aggregations")
    buckets = _aggregation(aggregations["projects"], "GDC project aggregation", MAX_REVIEWED_GDC_GENE_MUTATION_PROJECT_BUCKETS)
    rows, seen = [], set()
    for index, item in enumerate(buckets):
        row = _project_bucket(item, index)
        if row["project_id"] in seen:
            _fail("GDC analysis repeats a project identity")
        seen.add(row["project_id"])
        rows.append(row)
    if sum(row["project_doc_count"] for row in rows) > total_count:
        _fail("GDC project counts exceed the exact native hit total")
    retained = {row["project_id"]: row for row in rows if row["project_id"] in REVIEWED_GDC_GENE_MUTATION_PROJECTS}
    omitted = sorted((row for row in rows if row["project_id"] not in retained), key=lambda row: row["project_id"])
    projects = []
    for lane, project_id in zip(("gbm", "lgg"), REVIEWED_GDC_GENE_MUTATION_PROJECTS):
        native = retained.get(project_id, {"project_id": project_id, "state": "project_bucket_missing", "case_count_with_mutation": None, "project_doc_count": None, "nested_genes_doc_count": None, "selected_genes_doc_count": None, "source_json_pointer": None})
        projects.append({"source_id": _SOURCE_ID, "lane": lane, "gene_id": REVIEWED_GDC_GENE_MUTATION_GENE_ID, "gene_symbol": REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL, **native})
    metadata = {"took_ms": took, "timed_out": False, "shards": dict(shards), "hits_total": total_count, "hits_total_relation": "eq", "project_doc_count_error_upper_bound": 0, "project_sum_other_doc_count": 0, "returned_project_bucket_count": len(rows), "omitted_non_glioma_project_bucket_count": len(omitted), "omitted_non_glioma_project_buckets_digest": content_digest(omitted), "source_response_digest": content_digest(response)}
    return projects, metadata


@dataclass(frozen=True, slots=True)
class ReviewedGdcGeneMutationRetrievalConfig:
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_GDC_GENE_MUTATION_TRANSPORT_ID
    transport_version: str = BUILTIN_GDC_GENE_MUTATION_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_GDC_GENE_MUTATION_TRANSPORT_CONFIG_DIGEST

    def __post_init__(self) -> None:
        _integer("GDC gene mutation timeout_ms", self.timeout_ms, 100, 30_000)
        for name in ("transport_id", "transport_version"):
            if not isinstance(getattr(self, name), str) or not _IDENTIFIER.fullmatch(getattr(self, name)):
                _fail("GDC gene mutation transport identity is invalid")
        _digest(self.transport_config_digest, "GDC gene mutation transport config digest")

    @property
    def request_limit(self) -> int:
        return 1

    def _payload(self) -> dict[str, Any]:
        return {"schema": REVIEWED_GDC_GENE_MUTATION_CONFIG_SCHEMA, "gene_id": REVIEWED_GDC_GENE_MUTATION_GENE_ID, "gene_symbol": REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL, "project_ids": list(REVIEWED_GDC_GENE_MUTATION_PROJECTS), "request_url": _url(), "request_limit": 1, "timeout_ms": self.timeout_ms, "transport_id": self.transport_id, "transport_version": self.transport_version, "transport_config_digest": self.transport_config_digest, "retention": _RETENTION, "credentials": "not_accepted"}

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> ReviewedGdcGeneMutationRetrievalConfig:
        value = _json_copy(raw)
        expected = set(cls().to_dict())
        _exact(value, expected, "GDC gene mutation config")
        config = cls(value["timeout_ms"], value["transport_id"], value["transport_version"], value["transport_config_digest"])
        if canonical_json(value) != canonical_json(config.to_dict()):
            _fail("GDC gene mutation config differs from its reviewed identity or digest")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedGdcGeneMutationRetrievalPlan:
    config: ReviewedGdcGeneMutationRetrievalConfig
    config_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedGdcGeneMutationRetrievalConfig) -> ReviewedGdcGeneMutationRetrievalPlan:
        if type(config) is not ReviewedGdcGeneMutationRetrievalConfig:
            _fail("GDC gene mutation plan requires an exact config")
        checked = ReviewedGdcGeneMutationRetrievalConfig.from_dict(config.to_dict())
        unsigned = {"schema": REVIEWED_GDC_GENE_MUTATION_PLAN_SCHEMA, "config": checked.to_dict(), "config_digest": checked.config_digest, "request_limit": 1, "scope": _SCOPE, "execution": "one_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(checked, checked.config_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_GDC_GENE_MUTATION_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "request_limit": 1, "scope": _SCOPE, "execution": "one_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted", "plan_digest": self.plan_digest}

    def validate(self) -> None:
        if canonical_json(self.to_dict()) != canonical_json(self.create(self.config).to_dict()):
            _fail("GDC gene mutation plan drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> ReviewedGdcGeneMutationRetrievalPlan:
        value = _json_copy(raw)
        _exact(value, {"schema", "config", "config_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"}, "GDC gene mutation plan")
        plan = cls.create(ReviewedGdcGeneMutationRetrievalConfig.from_dict(value["config"]))
        if canonical_json(value) != canonical_json(plan.to_dict()):
            _fail("GDC gene mutation plan differs from its reviewed identity or digest")
        return plan


def _bundle(projects: list[dict[str, Any]], metadata: dict[str, Any], timestamp: str) -> dict[str, Any]:
    projection = {"gene_id": REVIEWED_GDC_GENE_MUTATION_GENE_ID, "gene_symbol": REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL, "projects": projects, "aggregation_metadata": metadata}
    source = {"source_id": _SOURCE_ID, "authority": REVIEWED_GDC_GENE_MUTATION_AUTHORITY, "uri": _url(), "retrieved_at": timestamp, "content_sha256": content_digest(projection), "record_count": len(projects), "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
    observed = sum(row["state"] == "observed" for row in projects)
    unsigned = {"schema": REVIEWED_GDC_GENE_MUTATION_BUNDLE_SCHEMA, "scope": _SCOPE, "generated_at": timestamp, **projection, "sources": [source], "source_set_digest": content_digest([source]), "project_count": 2, "observed_count": observed, "completeness": "requested_counts_present" if observed == 2 else "missing_requested_counts", "denominator": None, "prevalence_calculated": False, "counts_pooled": False, "cohort_overlap": "unassessed", "entrez_crosswalk": "not_supplied", "scientific_support_established": False, "human_review_required": True, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
    return {**unsigned, "bundle_digest": content_digest(unsigned)}


def validate_reviewed_gdc_gene_mutation_bundle(bundle: Mapping[str, Any]) -> dict[str, Any]:
    """Validate the exact normalized aggregate artifact; digests do not authenticate a source."""
    value = _json_copy(bundle)
    if not isinstance(value, dict):
        _fail("GDC gene mutation bundle is not an object")
    timestamp = _timestamp(value.get("generated_at"), "GDC gene mutation bundle timestamp")
    projects = value.get("projects")
    metadata = value.get("aggregation_metadata")
    if not isinstance(projects, list) or len(projects) != 2 or not isinstance(metadata, dict):
        _fail("GDC gene mutation bundle has malformed project or aggregation metadata")
    _exact(metadata, {"took_ms", "timed_out", "shards", "hits_total", "hits_total_relation", "project_doc_count_error_upper_bound", "project_sum_other_doc_count", "returned_project_bucket_count", "omitted_non_glioma_project_bucket_count", "omitted_non_glioma_project_buckets_digest", "source_response_digest"}, "GDC normalized aggregation metadata")
    _integer("GDC took_ms", metadata["took_ms"], maximum=86_400_000)
    _digest(metadata["source_response_digest"], "GDC source response digest")
    omitted = _integer("GDC omitted project bucket count", metadata["omitted_non_glioma_project_bucket_count"], maximum=MAX_REVIEWED_GDC_GENE_MUTATION_PROJECT_BUCKETS)
    omitted_digest = _digest(metadata["omitted_non_glioma_project_buckets_digest"], "GDC omitted project bucket digest")
    if omitted == 0 and omitted_digest != content_digest([]):
        _fail("GDC empty omitted project set has an inconsistent digest")
    returned = _integer("GDC returned project bucket count", metadata["returned_project_bucket_count"], maximum=MAX_REVIEWED_GDC_GENE_MUTATION_PROJECT_BUCKETS)
    # Replay only retained native fields. Omitted non-glioma aggregates remain digest-only.
    replay_buckets, indices = [], set()
    for index, (lane, project_id) in enumerate(zip(("gbm", "lgg"), REVIEWED_GDC_GENE_MUTATION_PROJECTS)):
        row = _exact(projects[index], {"source_id", "lane", "gene_id", "gene_symbol", "project_id", "state", "case_count_with_mutation", "project_doc_count", "nested_genes_doc_count", "selected_genes_doc_count", "source_json_pointer"}, "GDC normalized project")
        if (row["source_id"], row["lane"], row["gene_id"], row["gene_symbol"], row["project_id"]) != (_SOURCE_ID, lane, REVIEWED_GDC_GENE_MUTATION_GENE_ID, REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL, project_id):
            _fail("GDC normalized project identity differs from its reviewed selector")
        state = row["state"]
        if state == "project_bucket_missing":
            if any(row[key] is not None for key in ("case_count_with_mutation", "project_doc_count", "nested_genes_doc_count", "selected_genes_doc_count", "source_json_pointer")):
                _fail("GDC missing project was given a fabricated count or source pointer")
            continue
        if state not in ("observed", "gene_bucket_missing"):
            _fail("GDC normalized project state is invalid")
        for name in ("project_doc_count", "nested_genes_doc_count", "selected_genes_doc_count"):
            _integer("GDC " + name, row[name])
        count = row["case_count_with_mutation"]
        if state == "observed":
            _integer("GDC case_count_with_mutation", count)
            pointer = row["source_json_pointer"]
            match = re.fullmatch(r"/aggregations/projects/buckets/(0|[1-9][0-9]{0,2})/genes/my_genes/gene_id/buckets/0/doc_count", pointer) if isinstance(pointer, str) else None
            if match is None or int(match[1]) >= returned or int(match[1]) in indices:
                _fail("GDC native source count pointer is invalid or duplicated")
            indices.add(int(match[1]))
        elif count is not None or row["source_json_pointer"] is not None:
            _fail("GDC missing gene was given a fabricated count or source pointer")
        native = {"key": project_id, "doc_count": row["project_doc_count"], "genes": {"doc_count": row["nested_genes_doc_count"], "my_genes": {"doc_count": row["selected_genes_doc_count"], "gene_id": {"doc_count_error_upper_bound": 0, "sum_other_doc_count": 0, "buckets": [] if count is None else [{"key": REVIEWED_GDC_GENE_MUTATION_GENE_ID, "doc_count": count}]}}}}
        _project_bucket(native, 0)
        replay_buckets.append(native)
    if returned != omitted + len(replay_buckets):
        _fail("GDC retained and omitted project bucket counts are inconsistent")
    mock = {"took": metadata["took_ms"], "timed_out": metadata["timed_out"], "_shards": metadata["shards"], "hits": {"total": {"value": metadata["hits_total"], "relation": metadata["hits_total_relation"]}, "max_score": None, "hits": []}, "aggregations": {"projects": {"doc_count_error_upper_bound": metadata["project_doc_count_error_upper_bound"], "sum_other_doc_count": metadata["project_sum_other_doc_count"], "buckets": replay_buckets}}}
    _projection(mock)
    expected = _bundle(projects, metadata, timestamp)
    if canonical_json(value) != canonical_json(expected):
        _fail("GDC gene mutation bundle differs from its normalized contract or digests")
    return expected


@dataclass(frozen=True, slots=True)
class ReviewedGdcGeneMutationRetrievalResult:
    _bundle: Mapping[str, Any] = field(repr=False)
    _receipt: Mapping[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(dict(self._bundle))

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(dict(self._receipt))

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": self.receipt, "retention": "aggregate_metadata_only"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_GDC_GENE_MUTATION_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_public_gene_mutation_aggregates"}


class ReviewedGdcGeneMutationRetrievalAdapter:
    """One credentialless aggregate query after exact plan validation and literal approval."""

    def __init__(self, config: ReviewedGdcGeneMutationRetrievalConfig, *, fetch: GdcGeneMutationFetcher | None = None) -> None:
        if type(config) is not ReviewedGdcGeneMutationRetrievalConfig:
            _fail("GDC gene mutation adapter requires an exact config")
        config = ReviewedGdcGeneMutationRetrievalConfig.from_dict(config.to_dict())
        if fetch is not None and (not callable(fetch) or config.transport_id == BUILTIN_GDC_GENE_MUTATION_TRANSPORT_ID):
            _fail("GDC gene mutation injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id, config.transport_version, config.transport_config_digest) != (BUILTIN_GDC_GENE_MUTATION_TRANSPORT_ID, BUILTIN_GDC_GENE_MUTATION_TRANSPORT_VERSION, BUILTIN_GDC_GENE_MUTATION_TRANSPORT_CONFIG_DIGEST):
            _fail("GDC gene mutation built-in transport identity is not exact")
        self.config, self._fetch = config, fetch

    def prepare(self) -> ReviewedGdcGeneMutationRetrievalPlan:
        return ReviewedGdcGeneMutationRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedGdcGeneMutationRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedGdcGeneMutationRetrievalResult:
        if type(plan) is not ReviewedGdcGeneMutationRetrievalPlan:
            _fail("GDC gene mutation execution requires an exact plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("GDC gene mutation execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("GDC gene mutation dispatch requires literal approval")
        timestamp = datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ") if retrieved_at is None else _timestamp(retrieved_at, "GDC gene mutation retrieved_at")
        try:
            raw = _builtin_fetch(_url(), self.config.timeout_ms) if self._fetch is None else self._fetch(_url())
        except ReviewedGdcGeneMutationRetrievalError:
            raise
        except Exception as error:
            raise ReviewedGdcGeneMutationRetrievalError("GDC gene mutation aggregate request failed") from error
        response, consumed = _parse_response(raw)
        projects, metadata = _projection(response)
        bundle = validate_reviewed_gdc_gene_mutation_bundle(_bundle(projects, metadata, timestamp))
        receipt_unsigned = {"schema": REVIEWED_GDC_GENE_MUTATION_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": bundle["source_set_digest"], "source_count": 1, "request_count": 1, "response_bytes": consumed, "project_count": 2, "observed_count": bundle["observed_count"], "completeness": bundle["completeness"], "retrieved_at": timestamp, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedGdcGeneMutationRetrievalResult(deepcopy(bundle), deepcopy(receipt))


def register_gdc_gene_mutation_operations(catalog: Any) -> Any:
    """Opt in to the real aggregate executor without changing the default source catalogue."""
    from .workflow_integrations import WorkflowOperation, WorkflowOperationResult

    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        config = ReviewedGdcGeneMutationRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000))
        adapter = ReviewedGdcGeneMutationRetrievalAdapter(config)
        result = adapter.execute(adapter.prepare(), approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at)
        return WorkflowOperationResult(result.bundle, result.receipt)

    integer = {"type": "integer", "minimum": 0, "maximum": 1_000_000_000_000}
    count_or_null = {"anyOf": [integer, {"type": "null"}]}
    digest = {"type": "string", "pattern": "^[0-9a-f]{64}$"}
    project_properties = {"source_id": {"const": _SOURCE_ID}, "lane": {"type": "string", "enum": ["gbm", "lgg"]}, "gene_id": {"const": REVIEWED_GDC_GENE_MUTATION_GENE_ID}, "gene_symbol": {"const": REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL}, "project_id": {"type": "string", "enum": list(REVIEWED_GDC_GENE_MUTATION_PROJECTS)}, "state": {"type": "string", "enum": ["observed", "project_bucket_missing", "gene_bucket_missing"]}, "case_count_with_mutation": count_or_null, "project_doc_count": count_or_null, "nested_genes_doc_count": count_or_null, "selected_genes_doc_count": count_or_null, "source_json_pointer": {"anyOf": [{"type": "string", "maxLength": 256}, {"type": "null"}]}}
    source_properties = {"source_id": {"const": _SOURCE_ID}, "authority": {"const": REVIEWED_GDC_GENE_MUTATION_AUTHORITY}, "uri": {"const": _url()}, "retrieved_at": {"type": "string"}, "content_sha256": digest, "record_count": {"const": 2}, "provider": {"const": "none"}, "credentials": {"const": "not_accepted"}, "limitations": {"type": "array", "items": {"type": "string"}}}
    metadata_properties = {"took_ms": {"type": "integer", "minimum": 0, "maximum": 86_400_000}, "timed_out": {"const": False}, "shards": {"type": "object", "required": ["total", "successful", "skipped", "failed"], "additionalProperties": False, "properties": {name: {"type": "integer", "minimum": 0, "maximum": 10_000} for name in ("total", "successful", "skipped", "failed")}}, "hits_total": integer, "hits_total_relation": {"const": "eq"}, "project_doc_count_error_upper_bound": {"const": 0}, "project_sum_other_doc_count": {"const": 0}, "returned_project_bucket_count": {"type": "integer", "minimum": 0, "maximum": 128}, "omitted_non_glioma_project_bucket_count": {"type": "integer", "minimum": 0, "maximum": 128}, "omitted_non_glioma_project_buckets_digest": digest, "source_response_digest": digest}
    properties = {"schema": {"const": REVIEWED_GDC_GENE_MUTATION_BUNDLE_SCHEMA}, "scope": {"const": _SCOPE}, "generated_at": {"type": "string"}, "gene_id": {"const": REVIEWED_GDC_GENE_MUTATION_GENE_ID}, "gene_symbol": {"const": REVIEWED_GDC_GENE_MUTATION_GENE_SYMBOL}, "projects": {"type": "array", "minItems": 2, "maxItems": 2, "items": {"type": "object", "required": list(project_properties), "additionalProperties": False, "properties": project_properties}}, "aggregation_metadata": {"type": "object", "required": list(metadata_properties), "additionalProperties": False, "properties": metadata_properties}, "sources": {"type": "array", "minItems": 1, "maxItems": 1, "items": {"type": "object", "required": list(source_properties), "additionalProperties": False, "properties": source_properties}}, "source_set_digest": digest, "bundle_digest": digest, "project_count": {"const": 2}, "observed_count": {"type": "integer", "minimum": 0, "maximum": 2}, "completeness": {"enum": ["requested_counts_present", "missing_requested_counts"]}, "denominator": {"type": "null"}, "prevalence_calculated": {"const": False}, "counts_pooled": {"const": False}, "cohort_overlap": {"const": "unassessed"}, "entrez_crosswalk": {"const": "not_supplied"}, "scientific_support_established": {"const": False}, "human_review_required": {"const": True}, "provider": {"const": "none"}, "credentials": {"const": "not_accepted"}, "limitations": {"type": "array", "items": {"type": "string"}}}
    catalog.register(WorkflowOperation("gdc.gene_mutation_counts", "Retrieve one fixed TP53 GDC aggregate query, retaining source-reported TCGA-GBM/LGG mutation-affected case counts and missingness; no individual records, denominator, prevalence, or cohort independence inference.", {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}}, {"type": "object", "required": list(properties), "additionalProperties": False, "properties": properties}, "gdc", read_only=True, implementation="live_adapter"), execute)
    return catalog
