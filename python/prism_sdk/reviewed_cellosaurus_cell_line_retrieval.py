"""One fixed Cellosaurus identity/curation metadata window, without biological-use inference.

Repository-defined extension: no available blueprint specifies this source contract. Admission
is grounded in one retained format capture, not a live adapter proof or complete API schema.
Unknown selected shapes refuse; missing metadata never becomes an authenticity/quality pass.
"""
from __future__ import annotations

import base64
import binascii
from collections.abc import Mapping
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import re
import secrets
from time import monotonic
from typing import Any, Callable

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_public_http import (REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, ReviewedPublicHttpClient,
    ReviewedPublicHttpConfig, ReviewedPublicHttpError, ReviewedPublicHttpResponse)

CELLOSAURUS_CELL_LINE_OPERATION_ID = "cellosaurus.glioblastoma_cell_line_metadata"
CELLOSAURUS_CELL_LINE_BUNDLE_SCHEMA = "aurora-reviewed-cellosaurus-cell-line-metadata/0.1"
CELLOSAURUS_CELL_LINE_RECEIPT_SCHEMA = "aurora-reviewed-cellosaurus-cell-line-receipt/0.1"
MAX_CELLOSAURUS_RESPONSE_BYTES = 524_288
MAX_CELLOSAURUS_RECORDS = 5
MAX_CELLOSAURUS_CITATIONS = 4096
MAX_CELLOSAURUS_BUNDLE_BYTES = 8_000_000
_URL = ("https://api.cellosaurus.org/search/cell-line?q=ox%3Asapiens+din%3Aglioblastoma"
    "&start=0&rows=5&format=json&fields=ac%2Cid%2Csy%2Cox%2Cdin%2Ccaution%2Cproblematic%2Cdt&sort=ac+asc")
# Preserve the exact installed route/config used for the separately retained format capture.
_ROUTE = "cellosaurus_glioblastoma_metadata_format"
_ROOT = "/Cellosaurus/cell-line-list"
_REQUIRED = {"accession-list", "name-list", "species-list", "disease-list"}
_OPTIONAL = {"comment-list", "created", "last-updated", "entry-version"}
_REFERENCE_OPTIONAL = {"category", "iri", "url"}
_DIGEST = re.compile(r"[a-f0-9]{64}\Z")
_DEFAULT_BINDING = content_digest({"adapter": CELLOSAURUS_CELL_LINE_BUNDLE_SCHEMA,
    "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, "fixed_url": _URL})
_LIMITATIONS = [
    "One fixed human-query glioblastoma-name first window (start0/rows5) is not exhaustive glioma, cell-line or disease coverage; source-filter correspondence remains unestablished.",
    "Required native NCBI_TaxID:9606 presence is a source assertion, not independently verified human-only species or donor identity; every admitted taxonomic member remains separate.",
    "NCIt disease accessions/labels remain source-native assertions; no concept equivalence, ontology expansion, confirmed glioblastoma classification or negative evidence is inferred from an empty list.",
    "Names, synonyms, primary accessions and comments remain ordered source assertions; no alias joins, scientific identity equivalence, authenticity, contamination, quality or suitability pass is established.",
    "Only the observed Problematic cell line comment category is admitted. Requested Caution serialization remains unobserved; unknown categories refuse the whole artifact rather than disappear.",
    "Missing, null, empty and observed optional metadata are distinct. Missing comments do not establish absence of problems; null/empty optional shapes are local synthetic admission policy, not observed API guarantees.",
    "Native URLs/IRIs are inert strings. No source links, subsequent window, release-info route, retries or fallback are followed.",
    "Record dates/entry-version retain exact strings and are not an API release, source snapshot identity or freshness guarantee. Totals, unreturned counts, completeness and omitted influence remain unknown.",
    "Selected fields exclude patient records, sequences, raw variants, therapies and culture/protocol fields, but a field allowlist cannot prove semantic absence of incidental free-text context.",
    "Retained raw bytes, transport declarations, native pointers and full deterministic replay support local consistency, not authenticated source truth, live adapter execution, scientific validity or human review.",
    "No patient/donor/publication/cohort/assay denominator, clinical or laboratory action is authorized. Distributed quota enforcement and automatic waiting are not implemented.",
]


class CellosaurusCellLineRetrievalError(ArgumentError):
    """A refused request/native shape never becomes an empty successful source window."""


def _fail(code: str) -> None:
    raise CellosaurusCellLineRetrievalError("cellosaurus_" + code)


@dataclass(frozen=True, slots=True)
class CellosaurusCellLineRetrievalConfig:
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


CellosaurusCellLineHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: CellosaurusCellLineRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=("api.cellosaurus.org",), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_CELLOSAURUS_RESPONSE_BYTES)


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
    if type(value) is dict:
        for key, item in value.items():
            if type(key) is not str or len(key.encode("utf-8")) > 128:
                _fail("json_key_bound")
            _tree(item, depth + 1, budget)
    elif type(value) is list:
        for item in value:
            _tree(item, depth + 1, budget)


def _safe(value: Any, maximum: int | None = None) -> Any:
    try:
        maximum = MAX_CELLOSAURUS_BUNDLE_BYTES if maximum is None else maximum
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            _fail("artifact_byte_bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except CellosaurusCellLineRetrievalError:
        raise
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        _fail("artifact_invalid")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            _fail("duplicate_json_field")
        result[key] = value
    return result


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_CELLOSAURUS_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8", "strict"), object_pairs_hook=_pairs,
            parse_constant=lambda _: _fail("nonfinite_json"))
        _tree(value)
    except CellosaurusCellLineRetrievalError:
        raise
    except (ValueError, UnicodeError, RecursionError):
        _fail("invalid_response_json")
    return _safe(value, MAX_CELLOSAURUS_RESPONSE_BYTES)


def _object(value: Any, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if type(value) is not dict or not required.issubset(value) or set(value) - required - optional:
        _fail("native_shape_changed")
    return value


def _text(value: Any, maximum: int = 4096) -> str:
    if type(value) is not str or not value or len(value.encode("utf-8")) > maximum or "\x00" in value:
        _fail("native_text_invalid")
    return value


def _array(value: Any, maximum: int, minimum: int = 0) -> list[Any]:
    if type(value) is not list or not minimum <= len(value) <= maximum:
        _fail("native_array_bound_or_shape")
    return value


def _native(payload: Any) -> list[dict[str, Any]]:
    wrapper = _object(payload, {"Cellosaurus"})
    rows = _array(_object(wrapper["Cellosaurus"], {"cell-line-list"})["cell-line-list"], MAX_CELLOSAURUS_RECORDS)
    previous = None
    for row in rows:
        _object(row, _REQUIRED, _OPTIONAL)
        accessions = _array(row["accession-list"], 1, 1)
        accession = _object(accessions[0], {"type", "value"})
        if accession["type"] != "primary":
            _fail("primary_accession_required")
        primary = _text(accession["value"], 128)
        if previous is not None and primary <= previous:
            _fail("primary_accessions_not_unique_ascending")
        previous = primary
        names = _array(row["name-list"], 128, 1)
        identifier_count = 0
        for name in names:
            _object(name, {"type", "value"})
            if type(name["type"]) is not str or name["type"] not in {"identifier", "synonym"}:
                _fail("native_name_type_unknown")
            identifier_count += name["type"] == "identifier"
            _text(name["value"])
        if identifier_count != 1:
            _fail("exactly_one_identifier_required")
        for field, database, maximum in (("species-list", "NCBI_TaxID", 16), ("disease-list", "NCIt", 32)):
            entries = _array(row[field], maximum)
            for entry in entries:
                _object(entry, {"database", "accession", "label"}, _REFERENCE_OPTIONAL)
                if entry["database"] != database:
                    _fail("native_reference_database_outside_scope")
                _text(entry["accession"], 128); _text(entry["label"])
                for key in _REFERENCE_OPTIONAL:
                    if entry.get(key) is not None:
                        _text(entry[key], 128 if key == "category" else 4096)
            if field == "species-list" and not any(entry["accession"] == "9606" for entry in entries):
                _fail("required_human_taxonomy_assertion_missing")
        for field in ("created", "last-updated", "entry-version"):
            if row.get(field) is not None:
                _text(row[field], 128)
        if row.get("comment-list") is not None:
            for comment in _array(row["comment-list"], 32):
                _object(comment, {"category", "value"})
                if comment["category"] != "Problematic cell line":
                    _fail("unobserved_or_unselected_comment_category")
                _text(comment["value"], 32_768)
    return rows


def _resolve(value: Any, pointer: str) -> Any:
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def _bundle(config: CellosaurusCellLineRetrievalConfig, *, body: bytes, retrieved_at: str,
            mode: str, binding: str) -> dict[str, Any]:
    payload = _parse(body)
    rows = _native(payload)
    if mode not in {"default_reviewed_public_http", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_http" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    transport = _http_config(config)
    response_digest = hashlib.sha256(body).hexdigest()
    citations, records = [], []

    def cite(pointer: str, primary: str | None) -> str:
        if len(citations) >= MAX_CELLOSAURUS_CITATIONS:
            _fail("citation_bound_no_truncation")
        value = _resolve(payload, pointer)
        unsigned = {"source_operation_id": CELLOSAURUS_CELL_LINE_OPERATION_ID, "source_uri": _URL,
            "retrieved_at": retrieved_at, "source_response_digest": response_digest, "primary_accession": primary,
            "artifact_pointer_root": "validated_cellosaurus_bundle", "artifact_pointer": "/source_inputs/0/native_payload" + pointer,
            "native_payload_pointer": pointer, "source_value": value, "source_value_digest": content_digest(value)}
        identifier = "cellosaurus_native:" + content_digest(unsigned)
        citations.append({"citation_id": identifier, **unsigned})
        return identifier

    def observations(row: dict[str, Any], fields: set[str], pointer: str, context: str, primary: str) -> dict[str, Any]:
        result = {}
        for field in sorted(fields):
            present = field in row
            path = pointer + "/" + field
            result[field] = {"state": "missing" if not present else "null" if row[field] is None else "observed",
                "value": row.get(field), "native_pointer": path if present else None,
                "value_citation_id": cite(path, primary) if present else None,
                "absence_context_citation_id": context if not present else None}
        return result

    root_citation = cite(_ROOT, None)
    for index, row in enumerate(rows):
        pointer = _ROOT + "/" + str(index)
        primary = row["accession-list"][0]["value"]
        context = cite(pointer, primary)
        nested = {}
        for field, fields in (("accession-list", {"type", "value"}), ("name-list", {"type", "value"}),
                ("species-list", {"database", "accession", "label"} | _REFERENCE_OPTIONAL),
                ("disease-list", {"database", "accession", "label"} | _REFERENCE_OPTIONAL), ("comment-list", {"category", "value"})):
            nested[field] = None if row.get(field) is None else []
            for position, entry in enumerate(row.get(field) or []):
                at = pointer + "/" + field + "/" + str(position)
                entry_context = cite(at, primary)
                nested[field].append({"native_pointer": at, "citation_id": entry_context,
                    "observations": observations(entry, fields, at, entry_context, primary)})
        records.append({"primary_accession": primary, "identifier": next(name["value"] for name in row["name-list"] if name["type"] == "identifier"),
            "native_pointer": pointer, "citation_id": context,
            "observations": observations(row, _REQUIRED | _OPTIONAL, pointer, context, primary), "array_entries": nested,
            "source_query_disease_correspondence": "unestablished", "source_identity_equivalence": "unestablished",
            "problem_absence_established": False, "authenticity_or_quality_or_suitability_assessed": False})
    response = CellosaurusCellLineHttpResponse(200, body, _URL, len(body), response_digest,
        REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, transport.config_digest, "application/json")
    source = {"url": _URL, "http_method": "GET", "response_body_base64": base64.b64encode(body).decode("ascii"),
        "response_bytes": len(body), "response_digest": response_digest, "native_payload": payload,
        "payload_digest": content_digest(payload), "typed_transport_metadata": response.to_metadata()}
    receipt = {"schema": CELLOSAURUS_CELL_LINE_RECEIPT_SCHEMA, "source_id": "cellosaurus", "source_uri": _URL,
        "retrieved_at": retrieved_at, "source_request_count": 1, "http_method": "GET", "response_digest": response_digest,
        "response_bytes": len(body), "returned_record_count": len(records), "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": CELLOSAURUS_CELL_LINE_BUNDLE_SCHEMA, "source_id": "cellosaurus", "retrieved_at": retrieved_at,
        "query": {"q": "ox:sapiens din:glioblastoma", "start": 0, "rows": MAX_CELLOSAURUS_RECORDS, "format": "json",
            "fields": ["ac", "id", "sy", "ox", "din", "caution", "problematic", "dt"], "sort": "ac asc", "timeout_ms": config.timeout_ms},
        "source_inputs": [source], "records": records, "receipts": [receipt], "citation_index": citations,
        "connection_citation_id": root_citation,
        "coverage": {"returned_record_count": len(records), "reported_total_count": None, "unreturned_count": None,
            "page_count": None, "pagination_followed": False, "completeness": "unknown", "omitted_influence": "unknown",
            "source_release_id": None, "source_snapshot_identity": "unestablished", "source_filter_correspondence": "unestablished",
            "discarded_records": 0, "omitted_local_citations": 0, "local_omission_scope": "admitted_window_only",
            "patient_or_donor_count": None, "cohort_or_assay_denominator": None, "independent_publication_count": None},
        "omissions": [{"group": group, "omitted_count": None, "influence": "unknown"} for group in
            ("other_records_and_pages", "other_glioma_terms_and_ontology_descendants", "unselected_fields_and_comment_categories",
             "source_release_identity", "independent_identity_authenticity_quality_or_suitability_assessment")],
        "transport": {"mode": mode, "binding_id": binding, "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
            "config_digest": transport.config_digest, "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_http" else "caller_owned",
            "automatic_retry": False},
        "source_origin_authentication": "unestablished", "scientific_support_established": False, "human_review_required": True,
        "clinical_actions_authorized": False, "laboratory_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def retrieve_cellosaurus_glioblastoma_cell_line_metadata(config: CellosaurusCellLineRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., CellosaurusCellLineHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher,
        binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: CellosaurusCellLineRetrievalConfig | None, *, approve_source_dispatch: bool,
        retrieved_at: str | None, fetcher: Callable[..., CellosaurusCellLineHttpResponse] | None, binding: str) -> dict[str, Any]:
    config = CellosaurusCellLineRetrievalConfig() if config is None else config
    if type(config) is not CellosaurusCellLineRetrievalConfig:
        _fail("typed_config_required")
    config = CellosaurusCellLineRetrievalConfig(config.timeout_ms)
    if approve_source_dispatch is not True:
        _fail("source_approval_required")
    at, transport = _clock(retrieved_at), _http_config(config)
    expected_config, started = transport.config_digest, monotonic()
    try:
        if fetcher is None:
            response = ReviewedPublicHttpClient(transport).get(_ROUTE, approve_source_dispatch=True)
        else:
            if not callable(fetcher):
                _fail("fetcher_invalid")
            response = fetcher(_URL, http_config=transport)
        if type(response) is not CellosaurusCellLineHttpResponse:
            _fail("typed_response_required")
        response = CellosaurusCellLineHttpResponse(response.status, response.body, response.request_url, response.response_bytes,
            response.body_sha256, response.transport_policy_digest, response.config_digest, response.content_type)
        if response.config_digest != expected_config or response.transport_policy_digest != REVIEWED_PUBLIC_HTTP_POLICY_DIGEST or response.content_type != "application/json":
            _fail("response_transport_metadata_changed")
    except (CellosaurusCellLineRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise CellosaurusCellLineRetrievalError("cellosaurus_source_request_failed") from None
    if monotonic() - started > config.timeout_ms / 1000:
        _fail("source_elapsed_deadline_exceeded")
    if response.request_url != _URL:
        _fail("response_scope_changed")
    return _bundle(config, body=response.body, retrieved_at=at,
        mode="default_reviewed_public_http" if fetcher is None else "caller_supplied", binding=binding)


def validate_cellosaurus_cell_line_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = CellosaurusCellLineRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        encoded = actual["source_inputs"][0]["response_body_base64"]
        if type(encoded) is not str:
            _fail("replay_bytes_invalid")
        body = base64.b64decode(encoded.encode("ascii"), validate=True)
        if base64.b64encode(body).decode("ascii") != encoded:
            _fail("replay_bytes_not_canonical")
        expected = _bundle(config, body=body, retrieved_at=_clock(actual["retrieved_at"]),
            mode=actual["transport"]["mode"], binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError, AttributeError, UnicodeError, binascii.Error):
        _fail("replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected):
        _fail("bundle_does_not_replay")
    return expected


def register_cellosaurus_cell_line_operations(catalog: Any, *, fetcher: Callable[..., CellosaurusCellLineHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if type(catalog) is not WorkflowOperationCatalog or fetcher is not None and not callable(fetcher):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(CellosaurusCellLineRetrievalConfig(arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": CELLOSAURUS_CELL_LINE_OPERATION_ID,
            "bundle_digest": bundle["bundle_digest"], "scientific_support_established": False})
    catalog.register(WorkflowOperation(CELLOSAURUS_CELL_LINE_OPERATION_ID,
        "Read one fixed public Cellosaurus human-query glioblastoma-name identity/curation metadata window; source names/comments are not authenticity, quality, suitability, scientific support or patient/donor/cohort/assay denominators. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {
            "type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "records", "source_inputs", "coverage", "transport", "bundle_digest"],
         "properties": {"schema": {"const": CELLOSAURUS_CELL_LINE_BUNDLE_SCHEMA}, "source_id": {"const": "cellosaurus"},
            "query": {"type": "object"}, "records": {"type": "array", "maxItems": MAX_CELLOSAURUS_RECORDS},
            "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "coverage": {"type": "object"},
            "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "cellosaurus", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog


__all__ = ["CELLOSAURUS_CELL_LINE_OPERATION_ID", "CELLOSAURUS_CELL_LINE_BUNDLE_SCHEMA", "CELLOSAURUS_CELL_LINE_RECEIPT_SCHEMA",
    "MAX_CELLOSAURUS_RESPONSE_BYTES", "MAX_CELLOSAURUS_RECORDS", "MAX_CELLOSAURUS_CITATIONS", "MAX_CELLOSAURUS_BUNDLE_BYTES",
    "CellosaurusCellLineRetrievalConfig", "CellosaurusCellLineRetrievalError", "CellosaurusCellLineHttpResponse",
    "retrieve_cellosaurus_glioblastoma_cell_line_metadata", "validate_cellosaurus_cell_line_bundle", "register_cellosaurus_cell_line_operations"]
