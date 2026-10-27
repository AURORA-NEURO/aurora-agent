"""Reviewed HGNC human TP53 nomenclature and source reference candidates.

Repository-defined extension: no available blueprint module specifies this HGNC contract.
One fixed public metadata query supplies no scientific identity join, sequences, raw variants,
individual records, clinical action or laboratory action. Non-ready source states stay explicit.
"""
from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import re
import secrets
import threading
from time import monotonic
from typing import Any, Callable

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_public_http import (REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, ReviewedPublicHttpClient,
    ReviewedPublicHttpConfig, ReviewedPublicHttpError, ReviewedPublicHttpResponse)

HGNC_GENE_OPERATION_ID = "hgnc.gene_references"
HGNC_GENE_BUNDLE_SCHEMA = "aurora-reviewed-hgnc-gene-references/0.1"
HGNC_GENE_RECEIPT_SCHEMA = "aurora-reviewed-hgnc-gene-receipt/0.1"
MAX_HGNC_RESPONSE_BYTES = 512_000
MAX_HGNC_RECORDS = 8
_URL = "https://rest.genenames.org/fetch/symbol/TP53"
_ROUTE = "human_TP53_nomenclature"
_FIELDS = ("hgnc_id", "symbol", "name", "status", "alias_symbol", "alias_name", "prev_symbol",
    "prev_name", "entrez_id", "ensembl_gene_id", "symbol_report_tag")
_ARRAY_FIELDS = {"alias_symbol", "alias_name", "prev_symbol", "prev_name"}
# These are documented stored metadata fields, never links the adapter follows. Their exact
# peripheral representation is retained without claiming semantic normalization.
_PERIPHERAL = set("agr bioparadigms_slc ccds_id cd cosmic curator_notes date_approved_reserved date_modified date_name_changed date_symbol_changed date_withdrawn ena enzyme_id gencc gene_group gene_group_id gtrnadb homeodb horde_id imgt intermediate_filament_db iuphar lncipedia lncrnadb location location_sortable locus_group locus_type lsdb mamit-trnadb mane_select merops mgd_id mirbase omim_id orphanet pseudogene.org pubmed_id refseq_accession rgd_id rna_central_id snornabase ucsc_id uniprot_ids uuid vega_id _version_".split())
_DIGEST = re.compile(r"[a-f0-9]{64}")
_RATE_LOCK, _NEXT_ADMISSION = threading.Lock(), 0.0
_LIMITATIONS = [
    "The fixed human TP53 symbol fetch returns source nomenclature/reference metadata; aliases and identifier agreement do not establish scientific equivalence or disease relevance.",
    "All admitted documents, identifier candidates, aliases, previous names, source status and peripheral metadata remain retained without selecting a preferred identity.",
    "Absent, withdrawn, ambiguous, mismatched-selector, missing-status and partially returned source responses are non-ready states, not an approved identity or negative biological evidence.",
    "Reported numFound and window accounting describe source metadata only; they do not establish scientific exhaustiveness or independently verified count exactness.",
    "Missing, explicit null and observed empty fields remain distinct; no numeric or alias coercion supplies a missing identifier.",
    "Peripheral stored fields retain bounded native values without type/semantic normalization; annotation URLs are inert and never followed.",
    "The default path admits at most ten starts per second per process by refusing early starts; cross-process quotas and automatic retry are not implemented.",
    "Exact UTF-8 response bytes, projections and digests support offline consistency checks, not authenticated source truth or independent scientific validation.",
    "No patient/sample data, raw variants, sequences, laboratory actions, clinical decisions or controlled-access material are requested or admitted.",
]


class HgncGeneRetrievalError(ArgumentError):
    """The fixed metadata request/artifact was refused without an empty fallback."""


def _fail(code: str) -> None:
    raise HgncGeneRetrievalError("hgnc_" + code)


@dataclass(frozen=True, slots=True)
class HgncGeneRetrievalConfig:
    gene_symbol: str = "TP53"
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.gene_symbol) is not str or self.gene_symbol != "TP53":
            _fail("fixed_TP53_symbol_required")
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


HgncGeneHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: HgncGeneRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=("rest.genenames.org",), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_HGNC_RESPONSE_BYTES)


def _clock(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        if type(value) is not str or datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").strftime("%Y-%m-%dT%H:%M:%SZ") != value:
            _fail("invalid_timestamp")
    except (ValueError, TypeError):
        _fail("invalid_timestamp")
    return value


def _tree(value: Any, depth: int = 0, budget: list[int] | None = None) -> None:
    budget = [30_000] if budget is None else budget
    budget[0] -= 1
    if depth > 20 or budget[0] < 0:
        _fail("json_tree_bound")
    if isinstance(value, dict):
        for key, item in value.items():
            if type(key) is not str or len(key) > 128:
                _fail("json_key_bound")
            _tree(item, depth+1, budget)
    elif isinstance(value, list):
        for item in value:
            _tree(item, depth+1, budget)


def _safe(value: Any, maximum: int = 3_000_000) -> Any:
    try:
        _tree(value)
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            _fail("artifact_byte_bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        _fail("artifact_invalid")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, item in pairs:
        if key in result:
            _fail("duplicate_json_field")
        result[key] = item
    return result


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_HGNC_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8", "strict"), object_pairs_hook=_pairs,
            parse_constant=lambda _value: _fail("nonfinite_json"))
    except (ValueError, UnicodeError, RecursionError):
        _fail("invalid_response_json")
    return _safe(value, MAX_HGNC_RESPONSE_BYTES)


def _object(value: Any, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if type(value) is not dict or not required.issubset(value) or set(value)-required-optional:
        _fail("native_shape_changed")
    return value


def _count(value: Any, maximum: int = 10**12) -> int:
    if type(value) is not int or not 0 <= value <= maximum:
        _fail("native_count_invalid")
    return value


def _text(value: Any, maximum: int = 16_000) -> str:
    if type(value) is not str or len(value.encode("utf-8")) > maximum or "\x00" in value:
        _fail("native_text_invalid")
    return value


def _observation(row: dict[str, Any], field: str, path: str) -> dict[str, Any]:
    if field not in row:
        return {"field": field, "state": "missing", "value": None, "native_pointer": None}
    value = row[field]
    state = "null" if value is None else "observed"
    if value is not None:
        if field in _ARRAY_FIELDS:
            if type(value) is not list or len(value) > 256:
                _fail("alias_array_bound")
            for item in value:
                _text(item)
            if not value:
                state = "empty"
        else:
            _text(value)
            if not value:
                state = "empty"
            patterns = {"hgnc_id": r"HGNC:[1-9][0-9]{0,11}", "entrez_id": r"[1-9][0-9]{0,11}",
                "ensembl_gene_id": r"ENSG[0-9]{11}"}
            if value and field in patterns and re.fullmatch(patterns[field], value) is None:
                _fail("native_identifier_invalid")
    return {"field": field, "state": state, "value": value, "native_pointer": path + "/" + field}


def _native(payload: Any) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    _object(payload, {"responseHeader", "response"})
    header = _object(payload["responseHeader"], {"status", "QTime"}, {"params"})
    if type(header["status"]) is not int or header["status"] != 0:
        _fail("source_status_not_success")
    _count(header["QTime"], 86_400_000)
    if "params" in header:
        params = header["params"]
        if type(params) is not dict or len(params) > 32:
            _fail("source_parameter_metadata_bound")
        for key, value in params.items():
            _text(key, 128)
            if type(value) is list:
                if len(value) > 32: _fail("source_parameter_metadata_bound")
                for item in value: _text(item, 4096)
            else:
                _text(value, 4096)
    response = _object(payload["response"], {"numFound", "start", "docs"}, {"numFoundExact"})
    total, start, docs = _count(response["numFound"]), _count(response["start"]), response["docs"]
    if type(docs) is not list or len(docs) > MAX_HGNC_RECORDS or start+len(docs) > total:
        _fail("source_document_window_invalid")
    if "numFoundExact" in response and type(response["numFoundExact"]) is not bool:
        _fail("source_total_exactness_invalid")
    records = []
    for index, row in enumerate(docs):
        _object(row, set(), set(_FIELDS)|_PERIPHERAL)
        path = f"/response/docs/{index}"
        observations = {field: _observation(row, field, path) for field in _FIELDS}
        peripheral = {key: value for key, value in row.items() if key not in _FIELDS}
        for value in peripheral.values():
            values = value if type(value) is list else [value]
            if len(values) > 512: _fail("peripheral_array_bound")
            for item in values:
                if type(item) is str: _text(item)
                elif type(item) is int: _count(item, 10**24)
                elif item is not None: _fail("peripheral_metadata_shape")
        symbol, status, identifier = (observations[field] for field in ("symbol", "status", "hgnc_id"))
        state = ("missing_identity" if identifier["state"] != "observed" else
            "missing_symbol" if symbol["state"] != "observed" else
            "selector_value_mismatch" if symbol["value"] != "TP53" else
            "missing_status" if status["state"] != "observed" else
            "withdrawn" if status["value"] == "Entry Withdrawn" else
            "unreviewed_status" if status["value"] != "Approved" else "approved_source_record")
        records.append({"record_index": index, "native_pointer": path, "state": state,
            "field_observations": observations, "peripheral_native_fields": peripheral,
            "peripheral_semantics": "retained_uninterpreted_metadata", "native_record": row})
    return payload, records


def _selection(payload: dict[str, Any], rows: list[dict[str, Any]]) -> str:
    response = payload["response"]
    if response.get("numFoundExact") is False:
        return "source_total_approximate"
    if response["start"] != 0 or response["numFound"] != len(rows):
        return "source_window_incomplete"
    if not rows:
        return "not_found"
    if len(rows) != 1:
        return "ambiguous_multiple_records"
    return "single_approved_source_record" if rows[0]["state"] == "approved_source_record" else rows[0]["state"]


_DEFAULT_BINDING = content_digest({"schema": HGNC_GENE_BUNDLE_SCHEMA, "url": _URL,
    "public_http_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, "rate_policy": "per_process_refuse_start_interval_below_0.1_seconds"})


def _bundle(config: HgncGeneRetrievalConfig, *, body: bytes, retrieved_at: str, mode: str, binding: str) -> dict[str, Any]:
    payload, rows = _native(_parse(body))
    if mode not in {"default_reviewed_public_https", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_https" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    transport, selection = _http_config(config), _selection(payload, rows)
    source = {"url": _URL, "http_status": 200, "response_body": body.decode("utf-8"),
        "response_bytes": len(body), "response_digest": hashlib.sha256(body).hexdigest(),
        "native_payload": payload, "payload_digest": content_digest(payload)}
    receipt = {"schema": HGNC_GENE_RECEIPT_SCHEMA, "source_id": "hgnc", "source_uri": _URL,
        "retrieved_at": retrieved_at, "source_request_count": 1, "response_bytes": len(body),
        "response_digest": source["response_digest"], "selection_state": selection,
        "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": HGNC_GENE_BUNDLE_SCHEMA, "source_id": "hgnc", "retrieved_at": retrieved_at,
        "query": {"field": "symbol", "term": "TP53", "species": "human_HGNC", "timeout_ms": config.timeout_ms},
        "records": rows, "source_inputs": [source], "receipts": [receipt],
        "coverage": {"selection_state": selection, "source_identity_ready": selection == "single_approved_source_record",
            "returned_record_count": len(rows), "reported_total_count": payload["response"]["numFound"],
            "source_start": payload["response"]["start"], "source_num_found_exact": payload["response"].get("numFoundExact"),
            "unreturned_reported_count": payload["response"]["numFound"]-len(rows),
            "unreturned_influence": "unknown" if payload["response"]["numFound"] != len(rows)
                or payload["response"].get("numFoundExact") is not True else "none_reported_unreturned",
            "retained_record_count": len(rows), "records_discarded": 0, "source_field_omissions": 0,
            "scientific_exhaustiveness": "unassessed", "identity_validation": "source_assertions_only",
            "scientific_identifier_equivalence": "unestablished", "source_release_id": None},
        "transport": {"mode": mode, "binding_id": binding, "transport_policy_digest": transport.transport_policy_digest,
            "config_digest": transport.config_digest, "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_https" else "caller_owned",
            "automatic_retry": False}, "human_review_required": True, "scientific_support_established": False,
        "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def _admit_start() -> None:
    global _NEXT_ADMISSION
    with _RATE_LOCK:
        now = monotonic()
        if now < _NEXT_ADMISSION:
            _fail("rate_admission_not_ready")
        _NEXT_ADMISSION = now + .1


def retrieve_hgnc_gene_references(config: HgncGeneRetrievalConfig | None = None, *, approve_source_dispatch: bool = False,
        retrieved_at: str | None = None, fetcher: Callable[..., HgncGeneHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
        fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: HgncGeneRetrievalConfig | None, *, approve_source_dispatch: bool, retrieved_at: str | None,
        fetcher: Callable[..., HgncGeneHttpResponse] | None, binding: str) -> dict[str, Any]:
    config = HgncGeneRetrievalConfig() if config is None else config
    if type(config) is not HgncGeneRetrievalConfig: _fail("typed_config_required")
    config = HgncGeneRetrievalConfig(config.gene_symbol, config.timeout_ms)
    if approve_source_dispatch is not True: _fail("source_approval_required")
    at, transport = _clock(retrieved_at), _http_config(config)
    expected_config, started = transport.config_digest, monotonic()
    try:
        if fetcher is None:
            _admit_start()
            response = ReviewedPublicHttpClient(transport).get(_ROUTE, approve_source_dispatch=True)
        else:
            if not callable(fetcher): _fail("fetcher_invalid")
            response = fetcher(_URL, http_config=transport)
        if type(response) is not HgncGeneHttpResponse: _fail("typed_response_required")
        response = HgncGeneHttpResponse(response.status, response.body, response.request_url, response.response_bytes,
            response.body_sha256, response.transport_policy_digest, response.config_digest, response.content_type)
        if (response.config_digest != expected_config or response.transport_policy_digest != REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
                or response.content_type != "application/json"):
            _fail("response_transport_metadata_changed")
    except (HgncGeneRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise HgncGeneRetrievalError("hgnc_source_request_failed") from None
    if monotonic()-started > config.timeout_ms/1000: _fail("source_elapsed_deadline_exceeded")
    if response.request_url != _URL or response.status != 200: _fail("response_scope_or_status")
    return _bundle(config, body=response.body, retrieved_at=at,
        mode="default_reviewed_public_https" if fetcher is None else "caller_supplied", binding=binding)


def validate_hgnc_gene_reference_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = HgncGeneRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        expected = _bundle(config, body=actual["source_inputs"][0]["response_body"].encode("utf-8", "strict"),
            retrieved_at=_clock(actual["retrieved_at"]), mode=actual["transport"]["mode"], binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError, AttributeError, UnicodeError):
        _fail("replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected): _fail("bundle_does_not_replay")
    return expected


def register_hgnc_gene_operations(catalog: Any, *, fetcher: Callable[..., HgncGeneHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if not isinstance(catalog, WorkflowOperationCatalog) or fetcher is not None and not callable(fetcher):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(HgncGeneRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": HGNC_GENE_OPERATION_ID,
            "bundle_digest": bundle["bundle_digest"], "scientific_identifier_equivalence": "unestablished"})
    catalog.register(WorkflowOperation(HGNC_GENE_OPERATION_ID,
        "Read fixed public HGNC human TP53 nomenclature, all alias/reference candidates and status; preserve absent/withdrawn/ambiguous states without scientific equivalence. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "records", "source_inputs", "coverage", "transport", "bundle_digest"],
            "properties": {"schema": {"const": HGNC_GENE_BUNDLE_SCHEMA}, "source_id": {"const": "hgnc"},
                "query": {"type": "object"}, "records": {"type": "array", "maxItems": MAX_HGNC_RECORDS},
                "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "coverage": {"type": "object"},
                "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "hgnc", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog
