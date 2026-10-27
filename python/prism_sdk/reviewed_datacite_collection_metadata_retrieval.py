"""One fixed DataCite DOI metadata object, without imaging or cohort authority.

Repository-defined extension: no available blueprint specifies this source contract.
The JSON:API example informs an explicit local admission policy, not full source-schema
conformance. Synthetic fixtures do not establish execution against the live service.
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

DATACITE_COLLECTION_METADATA_OPERATION_ID = "datacite.tcga_gbm_collection_metadata"
DATACITE_COLLECTION_METADATA_BUNDLE_SCHEMA = "aurora-reviewed-datacite-collection-metadata/0.1"
DATACITE_COLLECTION_METADATA_RECEIPT_SCHEMA = "aurora-reviewed-datacite-collection-metadata-receipt/0.1"
MAX_DATACITE_RESPONSE_BYTES = 524_288
MAX_DATACITE_CITATIONS = 64
MAX_DATACITE_BUNDLE_BYTES = 8_000_000
_DOI = "10.7937/K9/TCIA.2016.RNYFUYE9"
_URL = "https://api.datacite.org/dois/" + _DOI
_ROUTE = "datacite_tcga_gbm_collection_metadata"
_MEDIA_TYPES = ("application/json", "application/vnd.api+json")
_ASCII_LOWER = str.maketrans("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "abcdefghijklmnopqrstuvwxyz")
_FIELDS = ("doi", "titles", "publisher", "publicationYear", "types", "version", "metadataVersion",
    "schemaVersion", "state", "isActive", "created", "registered", "updated", "url")
_TEXT_FIELDS = {"version": 4096, "schemaVersion": 4096, "state": 128, "created": 128,
    "registered": 128, "updated": 128, "url": 4096}
_DIGEST = re.compile(r"[a-f0-9]{64}\Z")
_DEFAULT_BINDING = content_digest({"adapter": DATACITE_COLLECTION_METADATA_BUNDLE_SCHEMA,
    "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, "fixed_url": _URL,
    "accepted_content_types": list(_MEDIA_TYPES)})
_LIMITATIONS = [
    "One returned DataCite JSON:API object for a fixed DOI is not TCIA imaging execution or collection-wide coverage.",
    "The documented JSON:API example is not a complete source schema guarantee. Requiredness, optional types and bounds are explicit local admission policy; full native schema conformance remains unassessed.",
    "DOI correspondence uses ASCII A-Z equivalence only. Exact native strings remain separate; no trimming, URL unwrapping, Unicode casefolding, title-based disease equivalence or cross-source patient/cohort join is performed.",
    "Dataset/resource version, metadata record revision and metadata schema identifier are distinct source observations. None establishes a current dataset release, snapshot identity or freshness.",
    "Missing, null, admitted empty and other native values remain distinct. Publisher strings and complete objects retain their exact values; unknown publisher keys are not interpreted or silently removed.",
    "Nonfindable/inactive states remain source observations, not empty success, missingness, absence of research evidence or quality decisions.",
    "URLs, content/media links, relationships, subject keywords and related identifiers are inert retained metadata. No linked resource, history, subject, image, series or media is requested.",
    "All admitted raw response bytes remain retained; semantics of unprojected fields and omitted influence are unknown. Complete JSON retention is not complete scientific interpretation.",
    "No patient, subject, image, series, independent publication, study or cohort denominator is established by one metadata record.",
    "Raw reconstruction and hashes support local consistency only; they authenticate neither source truth nor caller clock/content-type/transport declarations. A coherent whole-source replacement remains possible.",
    "Free-text metadata and a fixed route do not prove semantic absence of incidental context. No scientific validity, clinical/laboratory action, imaging access or human-review completion follows from this bundle.",
    "Default transport and caller-supplied fixtures have distinct provenance. Descriptor live_adapter labels describe code paths, not measured live conformance; distributed quotas and automatic waiting are not implemented.",
]


class DataCiteCollectionMetadataRetrievalError(ArgumentError):
    """An unsupported singleton response refuses as a whole rather than becoming empty data."""


def _fail(code: str) -> None:
    raise DataCiteCollectionMetadataRetrievalError("datacite_" + code)


@dataclass(frozen=True, slots=True)
class DataCiteCollectionMetadataRetrievalConfig:
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


DataCiteCollectionMetadataHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: DataCiteCollectionMetadataRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=("api.datacite.org",), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_DATACITE_RESPONSE_BYTES,
        accepted_content_types=_MEDIA_TYPES)


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
            if type(key) is not str or len(key.encode("utf-8")) > 256:
                _fail("json_key_bound")
            _tree(item, depth + 1, budget)
    elif type(value) is list:
        for item in value:
            _tree(item, depth + 1, budget)


def _safe(value: Any, maximum: int | None = None) -> Any:
    try:
        maximum = MAX_DATACITE_BUNDLE_BYTES if maximum is None else maximum
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            _fail("artifact_byte_bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except DataCiteCollectionMetadataRetrievalError:
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
    if type(body) is not bytes or not 1 <= len(body) <= MAX_DATACITE_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        payload = json.loads(body.decode("utf-8", "strict"), object_pairs_hook=_pairs,
            parse_constant=lambda _: _fail("nonfinite_json"))
        _tree(payload)
    except DataCiteCollectionMetadataRetrievalError:
        raise
    except (ValueError, UnicodeError, RecursionError):
        _fail("invalid_response_json")
    return _safe(payload, MAX_DATACITE_RESPONSE_BYTES)


def _object(value: Any, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if type(value) is not dict or not required.issubset(value) or set(value) - required - optional:
        _fail("native_shape_changed")
    return value


def _text(value: Any, maximum: int, *, empty: bool = False) -> str:
    if type(value) is not str or (not empty and not value) or len(value.encode("utf-8")) > maximum or "\x00" in value:
        _fail("native_text_invalid")
    return value


def _native(payload: Any) -> tuple[dict[str, Any], dict[str, Any]]:
    wrapper = _object(payload, {"data"}, {"links", "meta", "jsonapi"})
    row = _object(wrapper["data"], {"id", "type", "attributes"}, {"relationships", "links", "meta"})
    for obj, fields in ((wrapper, ("links", "meta", "jsonapi")), (row, ("relationships", "links", "meta"))):
        if any(key in obj and obj[key] is not None and type(obj[key]) is not dict for key in fields):
            _fail("inert_wrapper_metadata_shape_changed")
    attributes = row["attributes"]
    if type(attributes) is not dict or "doi" not in attributes or row["type"] != "dois":
        _fail("singleton_identity_required")
    for value in (row["id"], attributes["doi"]):
        if _text(value, 256).translate(_ASCII_LOWER) != _DOI.translate(_ASCII_LOWER):
            _fail("fixed_doi_correspondence_required")
    for key, maximum in _TEXT_FIELDS.items():
        if attributes.get(key) is not None:
            _text(attributes[key], maximum, empty=True)
    publisher = attributes.get("publisher")
    if publisher is not None:
        if type(publisher) is str:
            _text(publisher, 32_768, empty=True)
        elif type(publisher) is not dict:
            _fail("publisher_string_or_object_required")
    if attributes.get("types") is not None and type(attributes["types"]) is not dict:
        _fail("types_object_required")
    if attributes.get("titles") is not None:
        titles = attributes["titles"]
        if type(titles) is not list or len(titles) > 64:
            _fail("titles_array_bound_or_shape")
        for title in titles:
            if type(title) is not dict or "title" not in title:
                _fail("native_title_required")
            _text(title["title"], 32_768)
    for key, maximum in (("publicationYear", 9999), ("metadataVersion", 9_223_372_036_854_775_807)):
        if attributes.get(key) is not None and (type(attributes[key]) is not int or not 0 <= attributes[key] <= maximum):
            _fail("native_integer_bound_or_shape")
    if attributes.get("isActive") is not None and type(attributes["isActive"]) is not bool:
        _fail("native_active_boolean_required")
    return row, attributes


def _resolve(value: Any, pointer: str) -> Any:
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def _bundle(config: DataCiteCollectionMetadataRetrievalConfig, *, body: bytes, retrieved_at: str,
        content_type: str, mode: str, binding: str) -> dict[str, Any]:
    payload = _parse(body)
    row, attributes = _native(payload)
    if mode not in {"default_reviewed_public_http", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_http" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    if content_type not in _MEDIA_TYPES:
        _fail("response_content_type_outside_contract")
    transport = _http_config(config)
    response_digest = hashlib.sha256(body).hexdigest()
    citations = []

    def cite(pointer: str) -> str:
        if len(citations) >= MAX_DATACITE_CITATIONS:
            _fail("citation_bound_no_truncation")
        source_value = _resolve(payload, pointer)
        unsigned = {"source_operation_id": DATACITE_COLLECTION_METADATA_OPERATION_ID, "source_uri": _URL,
            "retrieved_at": retrieved_at, "source_response_digest": response_digest, "native_doi": attributes["doi"],
            "artifact_pointer_root": "validated_datacite_bundle", "artifact_pointer": "/source_inputs/0/native_payload" + pointer,
            "native_payload_pointer": pointer, "source_value": source_value, "source_value_digest": content_digest(source_value)}
        identifier = "datacite_native:" + content_digest(unsigned)
        citations.append({"citation_id": identifier, **unsigned})
        return identifier

    record_citation = cite("/data")
    attributes_citation = cite("/data/attributes")
    identity_citations = {key: cite("/data/" + key) for key in ("id", "type")}
    observations = {}
    for key in _FIELDS:
        present = key in attributes
        pointer = "/data/attributes/" + key
        observations[key] = {"state": "missing" if not present else "null" if attributes[key] is None else "observed",
            "value": attributes.get(key), "native_pointer": pointer if present else None,
            "value_citation_id": cite(pointer) if present else None,
            "absence_context_citation_id": attributes_citation if not present else None}
    record = {"native_id": row["id"], "native_type": row["type"], "native_doi": attributes["doi"],
        "citation_id": record_citation, "identity_citation_ids": identity_citations,
        "identity_comparison": "fixed_doi_ascii_A_to_Z_equivalence_only_native_strings_retained",
        "observations": observations, "dataset_version_observation": observations["version"],
        "metadata_revision_observation": observations["metadataVersion"], "metadata_schema_observation": observations["schemaVersion"],
        "unprojected_native_attribute_keys": sorted(set(attributes) - set(_FIELDS)),
        "native_record_completeness": "unassessed", "scientific_quality_assessed": False}
    typed = DataCiteCollectionMetadataHttpResponse(200, body, _URL, len(body), response_digest,
        REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, transport.config_digest, content_type)
    source = {"url": _URL, "http_method": "GET", "response_body_base64": base64.b64encode(body).decode("ascii"),
        "response_bytes": len(body), "response_digest": response_digest, "native_payload": payload,
        "payload_digest": content_digest(payload), "typed_transport_metadata": typed.to_metadata()}
    receipt = {"schema": DATACITE_COLLECTION_METADATA_RECEIPT_SCHEMA, "source_id": "datacite", "source_uri": _URL,
        "retrieved_at": retrieved_at, "source_request_count": 1, "http_method": "GET", "response_digest": response_digest,
        "response_bytes": len(body), "returned_record_count": 1, "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": DATACITE_COLLECTION_METADATA_BUNDLE_SCHEMA, "source_id": "datacite", "retrieved_at": retrieved_at,
        "query": {"doi": _DOI, "request_url": _URL, "timeout_ms": config.timeout_ms}, "source_inputs": [source],
        "records": [record], "receipts": [receipt], "citation_index": citations,
        "coverage": {"returned_record_count": 1, "count_scope": "one_returned_datacite_metadata_object",
            "native_response_schema_conformance": "unassessed", "source_metadata_record_completeness": "unknown",
            "dataset_release_id": None, "source_snapshot_identity": "unestablished", "completeness": "unknown",
            "omitted_influence": "unknown", "pagination_followed": False, "linked_resource_requests": 0,
            "discarded_records": 0, "omitted_local_citations": 0, "patient_or_subject_count": None,
            "image_or_series_count": None, "cohort_or_independent_study_count": None, "independent_publication_count": None},
        "omissions": [{"group": group, "omitted_count": None, "influence": "unknown"} for group in
            ("other_doi_or_collection_records", "dataset_versions_history_and_current_release",
             "unprojected_native_attribute_semantics", "linked_subject_image_series_and_media_resources",
             "independent_scientific_quality_access_rights_or_denominator_validation")],
        "transport": {"mode": mode, "binding_id": binding, "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
            "config_digest": transport.config_digest, "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_http" else "caller_owned",
            "automatic_retry": False},
        "source_origin_authentication": "unestablished", "scientific_support_established": False, "human_review_required": True,
        "clinical_actions_authorized": False, "laboratory_actions_authorized": False, "imaging_access_authorized": False,
        "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def retrieve_datacite_tcga_gbm_collection_metadata(config: DataCiteCollectionMetadataRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., DataCiteCollectionMetadataHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher,
        binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: DataCiteCollectionMetadataRetrievalConfig | None, *, approve_source_dispatch: bool,
        retrieved_at: str | None, fetcher: Callable[..., DataCiteCollectionMetadataHttpResponse] | None, binding: str) -> dict[str, Any]:
    config = DataCiteCollectionMetadataRetrievalConfig() if config is None else config
    if type(config) is not DataCiteCollectionMetadataRetrievalConfig:
        _fail("typed_config_required")
    config = DataCiteCollectionMetadataRetrievalConfig(config.timeout_ms)
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
        if type(response) is not DataCiteCollectionMetadataHttpResponse:
            _fail("typed_response_required")
        response = DataCiteCollectionMetadataHttpResponse(response.status, response.body, response.request_url, response.response_bytes,
            response.body_sha256, response.transport_policy_digest, response.config_digest, response.content_type)
        if (response.config_digest != expected_config or response.transport_policy_digest != REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
                or response.content_type not in _MEDIA_TYPES):
            _fail("response_transport_metadata_changed")
    except (DataCiteCollectionMetadataRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise DataCiteCollectionMetadataRetrievalError("datacite_source_request_failed") from None
    if monotonic() - started > config.timeout_ms / 1000:
        _fail("source_elapsed_deadline_exceeded")
    if response.request_url != _URL:
        _fail("response_scope_changed")
    return _bundle(config, body=response.body, retrieved_at=at, content_type=response.content_type,
        mode="default_reviewed_public_http" if fetcher is None else "caller_supplied", binding=binding)


def validate_datacite_collection_metadata_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = DataCiteCollectionMetadataRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        encoded = actual["source_inputs"][0]["response_body_base64"]
        if type(encoded) is not str:
            _fail("replay_bytes_invalid")
        body = base64.b64decode(encoded.encode("ascii"), validate=True)
        if base64.b64encode(body).decode("ascii") != encoded:
            _fail("replay_bytes_not_canonical")
        expected = _bundle(config, body=body, retrieved_at=_clock(actual["retrieved_at"]),
            content_type=actual["source_inputs"][0]["typed_transport_metadata"]["content_type"],
            mode=actual["transport"]["mode"], binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError, AttributeError, UnicodeError, binascii.Error):
        _fail("replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected):
        _fail("bundle_does_not_replay")
    return expected


def register_datacite_collection_metadata_operations(catalog: Any, *,
        fetcher: Callable[..., DataCiteCollectionMetadataHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if type(catalog) is not WorkflowOperationCatalog or fetcher is not None and not callable(fetcher):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)

    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(DataCiteCollectionMetadataRetrievalConfig(arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": DATACITE_COLLECTION_METADATA_OPERATION_ID,
            "bundle_digest": bundle["bundle_digest"], "scientific_support_established": False})

    catalog.register(WorkflowOperation(DATACITE_COLLECTION_METADATA_OPERATION_ID,
        "Read one fixed public DataCite DOI metadata object for the TCGA-GBM collection; retain native publisher and distinct dataset version/metadata revision/schema fields. This does not read TCIA images, subjects or media or establish scientific quality, completeness or denominators. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {
            "type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "records", "source_inputs", "citation_index", "coverage", "transport", "bundle_digest"],
            "properties": {"schema": {"const": DATACITE_COLLECTION_METADATA_BUNDLE_SCHEMA}, "source_id": {"const": "datacite"},
                "query": {"type": "object"}, "records": {"type": "array", "minItems": 1, "maxItems": 1},
                "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "citation_index": {"type": "array", "maxItems": MAX_DATACITE_CITATIONS},
                "coverage": {"type": "object"}, "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "datacite", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog


__all__ = ["DATACITE_COLLECTION_METADATA_OPERATION_ID", "DATACITE_COLLECTION_METADATA_BUNDLE_SCHEMA", "DATACITE_COLLECTION_METADATA_RECEIPT_SCHEMA",
    "MAX_DATACITE_RESPONSE_BYTES", "MAX_DATACITE_CITATIONS", "MAX_DATACITE_BUNDLE_BYTES",
    "DataCiteCollectionMetadataRetrievalConfig", "DataCiteCollectionMetadataRetrievalError", "DataCiteCollectionMetadataHttpResponse",
    "retrieve_datacite_tcga_gbm_collection_metadata", "validate_datacite_collection_metadata_bundle", "register_datacite_collection_metadata_operations"]
