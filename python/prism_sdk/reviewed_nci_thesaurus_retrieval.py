"""Bounded source-native NCIt glioma taxonomy, a repository-defined extension.

No detailed blueprint module for this public ontology reader is configured in this
checkout. Native ontology names and relations are terminology assertions, not cohort,
assay, causal, diagnostic or clinical evidence. Only one fixed concept request is made.
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
from .reviewed_public_http import (
    REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, ReviewedPublicHttpClient, ReviewedPublicHttpConfig,
    ReviewedPublicHttpError, ReviewedPublicHttpResponse,
)

NCI_THESAURUS_BUNDLE_SCHEMA = "aurora-reviewed-nci-thesaurus/0.1"
NCI_THESAURUS_RECEIPT_SCHEMA = "aurora-reviewed-nci-thesaurus-receipt/0.1"
NCI_THESAURUS_OPERATION = "nci_thesaurus.glioma_ontology"
NCI_THESAURUS_CONCEPT = "C3059"
MAX_NCI_THESAURUS_RESPONSE_BYTES = 1_000_000
MAX_NCI_THESAURUS_PART_RECORDS = 100
_HOST = "api-evsrest.nci.nih.gov"
_ROUTE = "glioma_concept"
_URL = ("https://api-evsrest.nci.nih.gov/api/v1/concept/ncit/C3059"
    "?include=definitions,children,parents,roles,associations&limit=100")
_PARTS = ("definitions", "children", "parents", "roles", "associations")
_OMITTED = ("synonyms", "properties", "descendants", "disjointWith", "history",
    "inverseAssociations", "inverseRoles", "maps", "paths", "extensions")
_BASE_FIELDS = {"uri", "code", "name", "terminology", "version", "conceptStatus",
    "source", "leaf", "active", "defining"}
_DEFINITION_FIELDS = {"uri", "definition", "highlight", "code", "type", "source", "qualifiers"}
_RELATION_FIELDS = {"uri", "code", "type", "relatedCode", "relatedName", "source", "highlight", "qualifiers"}
_QUALIFIER_FIELDS = {"uri", "code", "type", "value"}
_CODE = re.compile(r"C[1-9][0-9]{0,11}")
_DIGEST = re.compile(r"[a-f0-9]{64}")
_LIMITATIONS = [
    "This single fixed NCIt C3059 lookup supplies source-native glioma ontology assertions, not independently validated scientific evidence.",
    "Direct child and parent links retain their API field labels; no unstated relation type, transitive closure, cross-ontology equivalence or descendant coverage is inferred.",
    "Roles and associations retain native type, code, source, group and qualifiers; ontology relations are not measured biological or causal effects.",
    "Only the root concept's requested parts are read. Related concepts carry minimal source metadata, without fetching their definitions or descendants.",
    "Missing or null parts remain not_reported with an unknown count; an explicit empty array is observed empty_success. Neither establishes absence of a biological phenomenon.",
    "Known ct limit markers, unknown fields and malformed lists refuse the response; absence of a limit marker is not independent proof of taxonomy completeness.",
    "The source version is retained as returned by a current ncit lookup; no version-pinned release, cross-acquisition compatibility or WHO classification equivalence is asserted.",
    "No patients, samples, sequences, raw variants, measurements, cohort denominators, laboratory actions, diagnoses or clinical actions are requested.",
    "Raw bytes, digests and offline replay establish retained-artifact consistency, not authenticated source truth, terminology quality or scientific review.",
]


class NciThesaurusRetrievalError(ArgumentError):
    """The fixed ontology request or retained native artifact is inadmissible."""


def _fail(code: str) -> None:
    raise NciThesaurusRetrievalError("nci_thesaurus_" + code)


@dataclass(frozen=True, slots=True)
class NciThesaurusRetrievalConfig:
    concept_code: str = NCI_THESAURUS_CONCEPT
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.concept_code) is not str or self.concept_code != NCI_THESAURUS_CONCEPT:
            _fail("concept_outside_reviewed_scope")
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


NciThesaurusHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: NciThesaurusRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=(_HOST,), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_NCI_THESAURUS_RESPONSE_BYTES)


def _clock(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        if type(value) is not str or datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").strftime("%Y-%m-%dT%H:%M:%SZ") != value:
            _fail("invalid_utc_timestamp")
    except (TypeError, ValueError):
        _fail("invalid_utc_timestamp")
    return value


def _tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    budget = [150_000] if budget is None else budget
    budget[0] -= 1
    if depth > 20 or budget[0] < 0:
        _fail("json_tree_bound")
    if isinstance(value, dict):
        for key, child in value.items():
            if type(key) is not str or len(key) > 128:
                _fail("json_key_bound")
            _tree(child, depth=depth+1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _tree(child, depth=depth+1, budget=budget)


def _safe(value: Any, maximum: int = 5_000_000) -> Any:
    try:
        _tree(value)
        encoded = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(encoded.encode("utf-8")) > maximum:
            _fail("artifact_byte_bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except (TypeError, ValueError, UnicodeError, RecursionError, OverflowError):
        _fail("artifact_invalid")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            _fail("duplicate_json_field")
        result[key] = value
    return result


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_NCI_THESAURUS_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_pairs,
            parse_constant=lambda _value: _fail("nonfinite_json"))
        if not isinstance(value, dict):
            _fail("concept_object_required")
        return _safe(value, MAX_NCI_THESAURUS_RESPONSE_BYTES)
    except (UnicodeError, ValueError, RecursionError):
        _fail("invalid_response_json")


def _text(value: Any, maximum: int = 2048) -> str:
    if type(value) is not str or not value.strip() or len(value.encode("utf-8")) > maximum or "\x00" in value:
        _fail("invalid_source_text")
    return value


def _fields(row: Any, fields: set[str], required: set[str]) -> None:
    if not isinstance(row, dict):
        _fail("native_object_required")
    if "ct" in row:
        _fail("source_limit_marker")
    if set(row)-fields or not required.issubset(row):
        _fail("native_fields_outside_contract")


def _scalars(row: dict[str, Any], *, excluded: set[str] = frozenset()) -> None:
    for key, value in row.items():
        if key not in excluded and value is not None:
            _text(value, 16_384 if key == "definition" else 2048)


def _qualifiers(row: dict[str, Any]) -> None:
    if "qualifiers" not in row or row["qualifiers"] is None:
        return
    values = row["qualifiers"]
    if not isinstance(values, list) or len(values) > 32:
        _fail("qualifier_array_bound")
    for item in values:
        _fields(item, _QUALIFIER_FIELDS, {"type", "value"})
        _text(item["type"])
        _text(item["value"], 16_384)
        _scalars(item, excluded={"value"})


def _concept(row: Any, *, root: bool, version: str | None = None) -> None:
    _fields(row, _BASE_FIELDS | (set(_PARTS) if root else {"qualifiers"}), {"code", "name", "terminology", "version"})
    if _CODE.fullmatch(_text(row["code"], 64)) is None or row["terminology"] != "ncit":
        _fail("native_concept_identity")
    _text(row["name"])
    if re.fullmatch(r"[A-Za-z0-9_.-]{1,64}", _text(row["version"], 64)) is None:
        _fail("native_version")
    if root and (row["code"] != NCI_THESAURUS_CONCEPT or row["name"] != "Glioma"):
        _fail("root_identity_mismatch")
    if not root and (row["code"] == NCI_THESAURUS_CONCEPT or row["version"] != version):
        _fail("related_concept_identity_or_version")
    for key in ("leaf", "active", "defining"):
        if key in row and row[key] is not None and type(row[key]) is not bool:
            _fail("native_boolean_flag")
    _scalars(row, excluded=set(_PARTS) | {"leaf", "active", "defining", "qualifiers"})
    if not root:
        _qualifiers(row)


def _native(payload: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any]]:
    _concept(payload, root=True)
    relationships, coverage, hierarchy_codes = [], {}, {}
    for part in _PARTS:
        values = payload.get(part)
        if values is None:
            coverage[part] = {"state": "not_reported", "returned_count": None,
                "field_presence": "null" if part in payload else "absent"}
            continue
        if not isinstance(values, list) or len(values) > MAX_NCI_THESAURUS_PART_RECORDS:
            _fail("concept_part_array_bound")
        coverage[part] = {"state": "source_records" if values else "empty_success",
            "returned_count": len(values), "field_presence": "array"}
        seen = set()
        for index, row in enumerate(values):
            if part == "definitions":
                _fields(row, _DEFINITION_FIELDS, {"definition"})
                _text(row["definition"], 16_384)
                _scalars(row, excluded={"qualifiers"})
                _qualifiers(row)
            elif part in {"children", "parents"}:
                _concept(row, root=False, version=payload["version"])
                identity = row["code"]
                if identity in hierarchy_codes and hierarchy_codes[identity] != part:
                    _fail("contradictory_hierarchy_direction")
                hierarchy_codes[identity] = part
            else:
                _fields(row, _RELATION_FIELDS | ({"group"} if part == "roles" else set()),
                    {"code", "type", "relatedCode", "relatedName"})
                _scalars(row, excluded={"qualifiers"})
                for key in ("code", "type", "relatedCode", "relatedName"):
                    _text(row[key])
                if _CODE.fullmatch(row["relatedCode"]) is None:
                    _fail("native_related_code")
                _qualifiers(row)
            identity = row["code"] if part in {"children", "parents"} else content_digest(row)
            if identity in seen:
                _fail("duplicate_native_record")
            seen.add(identity)
            if part != "definitions":
                relationships.append({"source_part": part, "source_relationship_label": row["type"] if part in {"roles", "associations"} else part,
                    "root_code": NCI_THESAURUS_CONCEPT, "related_code": row["code"] if part in {"children", "parents"} else row["relatedCode"],
                    "related_preferred_name": row["name"] if part in {"children", "parents"} else row["relatedName"],
                    "source_pointer": f"/{part}/{index}", "native_record": row})
    return payload, relationships, coverage


_DEFAULT_BINDING = content_digest({"source": NCI_THESAURUS_BUNDLE_SCHEMA, "route": _URL,
    "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST})


def _bundle(config: NciThesaurusRetrievalConfig, *, retrieved_at: str, body: bytes,
            mode: str, binding: str) -> dict[str, Any]:
    payload, relationships, parts = _native(_parse(body))
    if mode not in {"default_reviewed_public_https", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_https" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    transport = _http_config(config)
    source = {"role": "glioma_ontology_concept", "url": _URL, "http_status": 200,
        "response_bytes": len(body), "response_digest": hashlib.sha256(body).hexdigest(),
        "response_body_base64": base64.b64encode(body).decode("ascii"),
        "native_payload": payload, "payload_digest": content_digest(payload)}
    receipt = {"schema": NCI_THESAURUS_RECEIPT_SCHEMA, "source_id": "nci_evs", "source_uri": _URL,
        "retrieved_at": retrieved_at, "response_bytes": source["response_bytes"], "response_digest": source["response_digest"],
        "payload_digest": source["payload_digest"], "source_request_count": 1,
        "concept_code": payload["code"], "source_version": payload["version"],
        "returned_relationship_count": len(relationships), "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": NCI_THESAURUS_BUNDLE_SCHEMA, "source_id": "nci_evs", "retrieved_at": retrieved_at,
        "query": {"terminology": "ncit", "concept_code": NCI_THESAURUS_CONCEPT, "include": list(_PARTS),
            "source_part_limit": MAX_NCI_THESAURUS_PART_RECORDS, "timeout_ms": config.timeout_ms},
        "concept": {"code": payload["code"], "preferred_name": payload["name"], "terminology": payload["terminology"],
            "source_version": payload["version"], "definitions": payload.get("definitions"), "source_pointer": ""},
        "relationships": relationships, "source_inputs": [source], "receipts": [receipt],
        "coverage": {"state": "source_ontology_assertions", "concept_parts": parts,
            "reported_total_count": None, "exhaustiveness": "not_independently_established",
            "source_limit_marker_present": False, "pagination": "not_followed_fixed_concept_lookup",
            "not_requested_concept_parts": list(_OMITTED), "related_concept_detail": "minimal_source_metadata_only",
            "omissions": [{"source_part": part, "reason": "not_requested", "omitted_count": None,
                "influence": "unknown"} for part in _OMITTED],
            "scientific_evidence_validation": "unassessed", "clinical_or_cohort_evidence": False},
        "transport": {"mode": mode, "binding_id": binding, "transport_policy_digest": transport.transport_policy_digest,
            "config_digest": transport.config_digest,
            "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_https" else "caller_owned",
            "automatic_retry": False}, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def retrieve_nci_thesaurus_glioma(config: NciThesaurusRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., NciThesaurusHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
        fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: NciThesaurusRetrievalConfig | None, *, approve_source_dispatch: bool,
        retrieved_at: str | None, fetcher: Callable[..., NciThesaurusHttpResponse] | None, binding: str) -> dict[str, Any]:
    config = NciThesaurusRetrievalConfig() if config is None else config
    if type(config) is not NciThesaurusRetrievalConfig:
        _fail("typed_config_required")
    # Revalidate even a frozen object deliberately modified by trusted application code.
    config = NciThesaurusRetrievalConfig(config.concept_code, config.timeout_ms)
    if approve_source_dispatch is not True:
        _fail("source_approval_required")
    at = _clock(retrieved_at)
    http_config = _http_config(config)
    started = monotonic()
    try:
        if fetcher is None:
            response = ReviewedPublicHttpClient(http_config).get(_ROUTE, approve_source_dispatch=True)
        else:
            if not callable(fetcher):
                _fail("fixture_fetcher_invalid")
            response = fetcher(_URL, http_config=http_config)
        if type(response) is not NciThesaurusHttpResponse:
            _fail("typed_response_required")
        response = NciThesaurusHttpResponse(response.status, response.body, response.request_url,
            response.response_bytes, response.body_sha256, response.transport_policy_digest,
            response.config_digest, response.content_type)
        if (response.request_url != _URL or response.config_digest != http_config.config_digest
                or response.transport_policy_digest != http_config.transport_policy_digest
                or response.content_type not in http_config.accepted_content_types):
            _fail("response_transport_metadata_mismatch")
    except (NciThesaurusRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise NciThesaurusRetrievalError("nci_thesaurus_source_request_failed") from None
    if monotonic()-started > config.timeout_ms/1000:
        _fail("source_elapsed_deadline_exceeded")
    return _bundle(config, retrieved_at=at, body=response.body,
        mode="default_reviewed_public_https" if fetcher is None else "caller_supplied", binding=binding)


def validate_nci_thesaurus_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = NciThesaurusRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        encoded = actual["source_inputs"][0]["response_body_base64"]
        if type(encoded) is not str or len(encoded) > 4*((MAX_NCI_THESAURUS_RESPONSE_BYTES+2)//3):
            _fail("retained_body_bound")
        body = base64.b64decode(encoded, validate=True)
        if base64.b64encode(body).decode("ascii") != encoded:
            _fail("retained_body_not_canonical_base64")
        expected = _bundle(config, retrieved_at=_clock(actual["retrieved_at"]), body=body,
            mode=actual["transport"]["mode"], binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError, binascii.Error, UnicodeError):
        _fail("native_replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected):
        _fail("bundle_does_not_replay")
    return expected


def register_nci_thesaurus_operations(catalog: Any, *, fetcher: Callable[..., NciThesaurusHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if not isinstance(catalog, WorkflowOperationCatalog) or (fetcher is not None and not callable(fetcher)):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(NciThesaurusRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": NCI_THESAURUS_OPERATION,
            "bundle_digest": bundle["bundle_digest"], "identity_validation": "source_assertions_only"})
    catalog.register(WorkflowOperation(NCI_THESAURUS_OPERATION,
        "Read fixed NCIt C3059 Glioma definitions and direct taxonomy/role/association assertions; ontology only, no cohort, assay, clinical or causal evidence. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "concept", "relationships", "source_inputs", "coverage", "transport", "bundle_digest"],
            "properties": {"schema": {"const": NCI_THESAURUS_BUNDLE_SCHEMA}, "source_id": {"const": "nci_evs"},
                "query": {"type": "object"}, "concept": {"type": "object"}, "relationships": {"type": "array", "maxItems": 400},
                "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "coverage": {"type": "object"},
                "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "nci_evs", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog
