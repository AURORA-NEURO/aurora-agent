"""Reviewed CIViC glioblastoma-name evidence metadata, without clinical interpretation.

Repository-defined extension: no available detailed blueprint specifies this source contract.
One fixed anonymous GraphQL query retains curation/publication identifiers and omissions.
It retrieves no descriptions, therapies, molecular profiles, variants, sequences or patients.
Source categories and evidence counts are not scientific support, clinical advice or denominators.
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
import threading
from time import monotonic
from typing import Any, Callable

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_public_http import ReviewedPublicHttpConfig
from .reviewed_public_graphql import (
    REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST, ReviewedPublicGraphqlClient,
    ReviewedPublicGraphqlConfig, ReviewedPublicGraphqlError, ReviewedPublicGraphqlResponse,
)

CIVIC_EVIDENCE_OPERATION_ID = "civic.glioblastoma_evidence_metadata"
CIVIC_EVIDENCE_BUNDLE_SCHEMA = "aurora-reviewed-civic-evidence-metadata/0.1"
CIVIC_EVIDENCE_RECEIPT_SCHEMA = "aurora-reviewed-civic-evidence-receipt/0.1"
MAX_CIVIC_RESPONSE_BYTES = 524_288
MAX_CIVIC_EVIDENCE_ITEMS = 25
CIVIC_EVIDENCE_OPERATION_NAME = "AuroraGlioblastomaEvidenceMetadataV1"
CIVIC_EVIDENCE_QUERY = ('query AuroraGlioblastomaEvidenceMetadataV1 { '
    'evidenceItems(diseaseName: "glioblastoma", status: ACCEPTED, first: 25, '
    'sortBy: {column: ID, direction: ASC}) { totalCount pageInfo { hasNextPage endCursor } '
    'nodes { id status flagged openRevisionCount evidenceType evidenceLevel evidenceDirection '
    'evidenceRating disease { id doid name } source { id citationId displayType } } } }')
_REQUEST_BODY = canonical_json({"operationName": CIVIC_EVIDENCE_OPERATION_NAME,
                               "variables": {}, "query": CIVIC_EVIDENCE_QUERY}).encode("utf-8")
CIVIC_EVIDENCE_REQUEST_BODY_SHA256 = hashlib.sha256(_REQUEST_BODY).hexdigest()
_URL, _ROUTE = "https://civicdb.org/api/graphql", "glioblastoma_evidence_metadata"
_DIGEST = re.compile(r"[a-f0-9]{64}\Z")
_REQUIRED = {"id", "status", "flagged", "openRevisionCount", "evidenceType",
             "evidenceLevel", "evidenceDirection", "source"}
_NULLABLE = {"disease", "evidenceRating"}
_ENUMS = {
    "status": frozenset({"ACCEPTED"}),
    "evidenceType": frozenset({"DIAGNOSTIC", "FUNCTIONAL", "ONCOGENIC", "PREDICTIVE", "PREDISPOSING", "PROGNOSTIC"}),
    "evidenceLevel": frozenset({"A", "B", "C", "D", "E"}),
    "evidenceDirection": frozenset({"DOES_NOT_SUPPORT", "NA", "SUPPORTS"}),
}
_RATE_LOCK, _NEXT_ADMISSION = threading.Lock(), 0.0
_DEFAULT_BINDING = content_digest({"adapter": CIVIC_EVIDENCE_BUNDLE_SCHEMA,
    "policy_digest": REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST,
    "request_body_sha256": CIVIC_EVIDENCE_REQUEST_BODY_SHA256,
    "admission": "one_start_per_second_per_process_refusal_no_retry/1"})
_LIMITATIONS = [
    "The fixed diseaseName substring filter is a glioblastoma-name evidence window, not all glioma terms, ontology descendants or exhaustive disease coverage.",
    "Exactly one first page is requested. Native totalCount, hasNextPage and endCursor remain source assertions; no cursor is followed and completeness remains unknown.",
    "Each evidence item remains separate. Shared source identifiers do not establish independent publications, patients, cohorts, assays or prevalence denominators.",
    "ACCEPTED status, evidence type, level, direction, rating, flags and revision counts are CIViC curation assertions, not validated scientific support or clinical recommendations.",
    "Missing, explicit null and observed values remain distinct. Missing nullable fields are not invented zero values or negative evidence.",
    "Other statuses, disease terms and unrequested fields have unknown omitted counts and influence. No decision sufficiency or zero influence is established.",
    "Descriptions, therapies, molecular profiles, variants, HGVS, sequences, patient records and source abstracts are not requested or admitted.",
    "The default path refuses starts less than one second apart per process; distributed quota enforcement and automatic waiting/retry are not implemented.",
    "Exact retained response/request bytes and native pointers support offline consistency checks, not authenticated source truth or scientific validation.",
    "Current API release/snapshot identity and deployed-schema parity are unestablished; no clinical or laboratory action is authorized.",
]


class CivicEvidenceRetrievalError(ArgumentError):
    """A refused fixed metadata request never becomes an empty successful evidence window."""


def _fail(code: str) -> None:
    raise CivicEvidenceRetrievalError("civic_" + code)


@dataclass(frozen=True, slots=True)
class CivicEvidenceRetrievalConfig:
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


CivicEvidenceHttpResponse = ReviewedPublicGraphqlResponse


def _http_config(config: CivicEvidenceRetrievalConfig) -> ReviewedPublicGraphqlConfig:
    return ReviewedPublicGraphqlConfig(ReviewedPublicHttpConfig(
        allowed_hosts=("civicdb.org",), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_CIVIC_RESPONSE_BYTES),
        {_ROUTE: _REQUEST_BODY})


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
            if type(key) is not str or len(key) > 128:
                _fail("json_key_bound")
            _tree(item, depth + 1, budget)
    elif type(value) is list:
        for item in value:
            _tree(item, depth + 1, budget)


def _safe(value: Any, maximum: int = 4_000_000) -> Any:
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
    if type(body) is not bytes or not 1 <= len(body) <= MAX_CIVIC_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8", "strict"), object_pairs_hook=_pairs,
                           parse_constant=lambda _: _fail("nonfinite_json"))
    except (ValueError, UnicodeError, RecursionError):
        _fail("invalid_response_json")
    return _safe(value, MAX_CIVIC_RESPONSE_BYTES)


def _object(value: Any, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if type(value) is not dict or not required.issubset(value) or set(value) - required - optional:
        _fail("native_shape_changed")
    return value


def _count(value: Any, minimum: int = 0) -> int:
    if type(value) is not int or not minimum <= value <= 2_147_483_647:
        _fail("native_integer_invalid")
    return value


def _text(value: Any, maximum: int = 4096, *, empty: bool = False) -> str:
    if type(value) is not str or len(value.encode("utf-8")) > maximum or "\x00" in value or (not empty and not value):
        _fail("native_text_invalid")
    return value


def _observation(row: dict[str, Any], field: str, path: str) -> dict[str, Any]:
    if field not in row:
        return {"state": "missing", "value": None, "native_pointer": None}
    value = row[field]
    return {"state": "null" if value is None else "observed", "value": value,
            "native_pointer": path + "/" + field}


def _native(payload: Any) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if type(payload) is dict and "errors" in payload:
        _fail("graphql_errors_present")
    _object(payload, {"data"})
    data = _object(payload["data"], {"evidenceItems"})
    connection = _object(data["evidenceItems"], {"totalCount", "pageInfo", "nodes"})
    total, nodes = _count(connection["totalCount"]), connection["nodes"]
    page = _object(connection["pageInfo"], {"hasNextPage"}, {"endCursor"})
    if type(page["hasNextPage"]) is not bool:
        _fail("native_pagination_invalid")
    if page.get("endCursor") is not None:
        _text(page["endCursor"], 4096)
    if (type(nodes) is not list or len(nodes) > MAX_CIVIC_EVIDENCE_ITEMS
            or len(nodes) != min(total, MAX_CIVIC_EVIDENCE_ITEMS)
            or page["hasNextPage"] is not (total > len(nodes))
            or (not nodes and page.get("endCursor") is not None)):
        _fail("native_page_accounting_invalid")
    records, previous = [], 0
    for index, row in enumerate(nodes):
        _object(row, _REQUIRED, _NULLABLE)
        identifier = _count(row["id"], 1)
        if identifier <= previous:
            _fail("native_ids_not_unique_ascending")
        previous = identifier
        for field, allowed in _ENUMS.items():
            if type(row[field]) is not str or row[field] not in allowed:
                _fail("native_enum_or_status_changed")
        if type(row["flagged"]) is not bool:
            _fail("native_flag_invalid")
        _count(row["openRevisionCount"])
        if row.get("evidenceRating") is not None:
            rating = _count(row["evidenceRating"], 1)
            if rating > 5:
                _fail("native_rating_outside_1_to_5")
        path = f"/data/evidenceItems/nodes/{index}"
        disease_fields = None
        if row.get("disease") is not None:
            disease = _object(row["disease"], {"id", "name"}, {"doid"})
            _count(disease["id"], 1); _text(disease["name"])
            if disease.get("doid") is not None:
                _text(disease["doid"], 256)
            disease_fields = {field: _observation(disease, field, path + "/disease")
                              for field in ("id", "doid", "name")}
        source = _object(row["source"], {"id", "citationId", "displayType"})
        _count(source["id"], 1); _text(source["citationId"], 256); _text(source["displayType"], 256)
        records.append({"evidence_id": identifier, "native_pointer": path,
            "observations": {field: _observation(row, field, path)
                             for field in sorted(_REQUIRED | _NULLABLE)},
            "disease_fields": disease_fields,
            "source_fields": {field: _observation(source, field, path + "/source")
                              for field in ("id", "citationId", "displayType")},
            "citation": {"source_input_index": 0, "source_uri": _URL,
                "native_pointer": path, "native_source_pointer": path + "/source",
                "evidence_id": identifier, "source_id": source["id"],
                "citation_id": source["citationId"], "display_type": source["displayType"]}})
    return connection, records


def _bundle(config: CivicEvidenceRetrievalConfig, *, body: bytes, retrieved_at: str,
            mode: str, binding: str) -> dict[str, Any]:
    payload = _parse(body)
    connection, records = _native(payload)
    if mode not in {"default_reviewed_public_graphql", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_graphql" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    transport = _http_config(config)
    source = {"url": _URL, "http_method": "POST", "http_status": 200,
        "request_body_utf8": _REQUEST_BODY.decode("utf-8"),
        "request_body_sha256": CIVIC_EVIDENCE_REQUEST_BODY_SHA256,
        "response_body_base64": base64.b64encode(body).decode("ascii"),
        "response_bytes": len(body), "response_digest": hashlib.sha256(body).hexdigest(),
        "native_payload": payload, "payload_digest": content_digest(payload)}
    nullable_missing = sum(record["observations"][field]["state"] == "missing"
                           for record in records for field in _NULLABLE)
    receipt = {"schema": CIVIC_EVIDENCE_RECEIPT_SCHEMA, "source_id": "civic", "source_uri": _URL,
        "retrieved_at": retrieved_at, "source_request_count": 1, "http_method": "POST",
        "request_body_sha256": CIVIC_EVIDENCE_REQUEST_BODY_SHA256,
        "response_bytes": len(body), "response_digest": source["response_digest"],
        "returned_evidence_item_count": len(records), "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": CIVIC_EVIDENCE_BUNDLE_SCHEMA, "source_id": "civic", "retrieved_at": retrieved_at,
        "query": {"operation_name": CIVIC_EVIDENCE_OPERATION_NAME, "query": CIVIC_EVIDENCE_QUERY,
            "variables": {}, "disease_name_substring": "glioblastoma", "status": "ACCEPTED",
            "first": MAX_CIVIC_EVIDENCE_ITEMS, "sort_by": {"column": "ID", "direction": "ASC"},
            "timeout_ms": config.timeout_ms, "request_body_sha256": CIVIC_EVIDENCE_REQUEST_BODY_SHA256},
        "records": records, "source_inputs": [source], "receipts": [receipt],
        "coverage": {"returned_evidence_item_count": len(records),
            "reported_filtered_total_count": connection["totalCount"],
            "unreturned_reported_evidence_item_count": connection["totalCount"] - len(records),
            "has_next_page": connection["pageInfo"]["hasNextPage"],
            "end_cursor": _observation(connection["pageInfo"], "endCursor", "/data/evidenceItems/pageInfo"),
            "pagination_followed": False, "page_accounting": "native_first_page_counts_consistent",
            "completeness": "unknown", "omitted_influence": "unknown",
            "missing_nullable_item_fields": nullable_missing, "discarded_evidence_items": 0,
            "source_release_id": None, "source_snapshot_identity": "unestablished",
            "scientific_support": "unestablished", "independent_publication_count": None,
            "patient_or_cohort_or_assay_denominator": None},
        "omissions": [{"group": group, "omitted_count": None, "influence": "unknown"} for group in
            ("other_evidence_statuses", "other_glioma_disease_terms_and_descendants", "later_pages",
             "descriptions", "therapies", "molecular_profiles", "variants_and_HGVS", "sequences",
             "patient_records", "source_abstracts", "source_publication_quality_and_retractions")],
        "transport": {"mode": mode, "binding_id": binding,
            "transport_policy_digest": REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST,
            "config_digest": transport.config_digest,
            "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_graphql" else "caller_owned",
            "automatic_retry": False},
        "human_review_required": True, "scientific_support_established": False,
        "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def _admit_start() -> None:
    global _NEXT_ADMISSION
    with _RATE_LOCK:
        now = monotonic()
        if now < _NEXT_ADMISSION:
            _fail("rate_admission_not_ready")
        _NEXT_ADMISSION = now + 1.0


def retrieve_civic_glioblastoma_evidence(config: CivicEvidenceRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., CivicEvidenceHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
                     fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: CivicEvidenceRetrievalConfig | None, *, approve_source_dispatch: bool,
              retrieved_at: str | None, fetcher: Callable[..., CivicEvidenceHttpResponse] | None,
              binding: str) -> dict[str, Any]:
    config = CivicEvidenceRetrievalConfig() if config is None else config
    if type(config) is not CivicEvidenceRetrievalConfig:
        _fail("typed_config_required")
    config = CivicEvidenceRetrievalConfig(config.timeout_ms)
    if approve_source_dispatch is not True:
        _fail("source_approval_required")
    at, transport = _clock(retrieved_at), _http_config(config)
    expected_config, started = transport.config_digest, monotonic()
    try:
        if fetcher is None:
            _admit_start()
            response = ReviewedPublicGraphqlClient(transport).post(_ROUTE, approve_source_dispatch=True)
        else:
            if not callable(fetcher):
                _fail("fetcher_invalid")
            response = fetcher(_URL, http_config=transport)
        if type(response) is not CivicEvidenceHttpResponse:
            _fail("typed_response_required")
        response = CivicEvidenceHttpResponse(response.status, response.body, response.request_url, response.response_bytes,
            response.body_sha256, response.transport_policy_digest, response.config_digest, response.content_type)
        if (response.config_digest != expected_config
                or response.transport_policy_digest != REVIEWED_PUBLIC_GRAPHQL_POLICY_DIGEST
                or response.content_type != "application/json"):
            _fail("response_transport_metadata_changed")
    except (CivicEvidenceRetrievalError, ReviewedPublicGraphqlError):
        raise
    except Exception:
        raise CivicEvidenceRetrievalError("civic_source_request_failed") from None
    if monotonic() - started > config.timeout_ms / 1000:
        _fail("source_elapsed_deadline_exceeded")
    if response.request_url != _URL or response.status != 200:
        _fail("response_scope_or_status")
    return _bundle(config, body=response.body, retrieved_at=at,
                   mode="default_reviewed_public_graphql" if fetcher is None else "caller_supplied", binding=binding)


def validate_civic_evidence_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = CivicEvidenceRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
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


def register_civic_evidence_operations(catalog: Any, *,
        fetcher: Callable[..., CivicEvidenceHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if type(catalog) is not WorkflowOperationCatalog or fetcher is not None and not callable(fetcher):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(CivicEvidenceRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": CIVIC_EVIDENCE_OPERATION_ID,
            "bundle_digest": bundle["bundle_digest"], "scientific_support_established": False})
    catalog.register(WorkflowOperation(CIVIC_EVIDENCE_OPERATION_ID,
        "Read one fixed public CIViC accepted glioblastoma-name evidence metadata page; source categories/counts are not clinical recommendations, scientific support or patient/cohort/assay denominators. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {
            "type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "records", "source_inputs",
            "coverage", "transport", "bundle_digest"], "properties": {
            "schema": {"const": CIVIC_EVIDENCE_BUNDLE_SCHEMA}, "source_id": {"const": "civic"},
            "query": {"type": "object"}, "records": {"type": "array", "maxItems": MAX_CIVIC_EVIDENCE_ITEMS},
            "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1},
            "coverage": {"type": "object"}, "transport": {"type": "object"},
            "bundle_digest": {"type": "string"}}},
        "civic", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog


__all__ = ["CIVIC_EVIDENCE_OPERATION_ID", "CIVIC_EVIDENCE_BUNDLE_SCHEMA", "CIVIC_EVIDENCE_RECEIPT_SCHEMA",
    "CIVIC_EVIDENCE_OPERATION_NAME", "CIVIC_EVIDENCE_QUERY", "CIVIC_EVIDENCE_REQUEST_BODY_SHA256",
    "MAX_CIVIC_RESPONSE_BYTES", "MAX_CIVIC_EVIDENCE_ITEMS", "CivicEvidenceRetrievalConfig",
    "CivicEvidenceRetrievalError", "CivicEvidenceHttpResponse", "retrieve_civic_glioblastoma_evidence",
    "validate_civic_evidence_bundle", "register_civic_evidence_operations"]

