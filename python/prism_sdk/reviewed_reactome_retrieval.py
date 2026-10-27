"""Reviewed human TP53 pathway annotation metadata, a repository-defined extension.

The fixed UniProt query is a source assertion, not a scientific identifier crosswalk or
evidence of pathway activity in glioma. No reactions, sequences, individual records,
measurements, enrichment tests or clinical/laboratory actions are requested.
"""
from __future__ import annotations

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

REACTOME_PATHWAY_BUNDLE_SCHEMA = "aurora-reviewed-reactome-pathways/0.1"
REACTOME_PATHWAY_RECEIPT_SCHEMA = "aurora-reviewed-reactome-pathway-receipt/0.1"
REACTOME_PATHWAY_OPERATION = "reactome.pathway_annotations"
MAX_REACTOME_RESPONSE_BYTES = 512_000
MAX_REACTOME_PATHWAYS = 256
_HOST = "reactome.org"
_URL = "https://reactome.org/ContentService/data/mapping/UniProt/P04637/pathways?species=9606"
_FIELDS = {"dbId", "displayName", "stId", "stIdVersion", "isInDisease", "isInferred", "maxDepth",
    "name", "releaseDate", "speciesName", "hasDiagram", "hasEHLD", "lastUpdatedDate", "schemaClass",
    "className", "doi", "releaseStatus"}
_REQUIRED = {"dbId", "displayName", "stId", "stIdVersion", "isInDisease", "isInferred", "name",
    "speciesName", "schemaClass", "className"}
_DIGEST = re.compile(r"[a-f0-9]{64}")
_LIMITATIONS = [
    "The fixed human UniProt P04637 mapping labels TP53 annotation context; it does not establish scientific identifier equivalence.",
    "The endpoint supplies lower-level source pathway annotations, not an exhaustive hierarchy or independent pathway relevance/quality assessment.",
    "General human annotation is not glioma-specific pathway activity, alteration, enrichment, causal effect or negative biological evidence.",
    "Source disease and inferred flags retain source semantics; neither is a glioma diagnosis or independent validation.",
    "A successful empty array differs from missing or failed acquisition; the source reports no total or completeness guarantee here.",
    "No sequences, reaction mechanisms, experimental interventions, assays, individual records, clinical or laboratory actions are requested.",
    "Retained JSON and self-computed digests establish internal consistency, not authenticated source truth; scientific human review is required.",
]


class ReactomeRetrievalError(ArgumentError):
    """A fixed annotation request or retained native artifact is inadmissible."""


def _fail(code: str) -> None:
    raise ReactomeRetrievalError("reactome_" + code)


@dataclass(frozen=True, slots=True)
class ReactomeRetrievalConfig:
    uniprot_accession: str = "P04637"
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.uniprot_accession) is not str or self.uniprot_accession != "P04637":
            _fail("accession_outside_reviewed_scope")
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


ReactomeHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: ReactomeRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=(_HOST,), reviewed_routes={"tp53_human_pathways": _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_REACTOME_RESPONSE_BYTES)


def _clock(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        if type(value) is not str or datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").strftime("%Y-%m-%dT%H:%M:%SZ") != value:
            _fail("invalid_utc_timestamp")
    except (ValueError, TypeError):
        _fail("invalid_utc_timestamp")
    return value


def _tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    budget = [30_000] if budget is None else budget
    budget[0] -= 1
    if depth > 20 or budget[0] < 0:
        _fail("json_tree_bound")
    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str) or len(key) > 128:
                _fail("json_key_bound")
            _tree(child, depth=depth+1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _tree(child, depth=depth+1, budget=budget)


def _safe(value: Any, maximum: int = 2_000_000) -> Any:
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


def _parse(body: bytes) -> Any:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_REACTOME_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_pairs,
            parse_constant=lambda _value: _fail("nonfinite_json"))
        return _safe(value, MAX_REACTOME_RESPONSE_BYTES)
    except (UnicodeError, ValueError, RecursionError):
        _fail("invalid_response_json")


def _text(value: Any, *, maximum: int = 2048) -> str:
    if type(value) is not str or not value.strip() or len(value.encode("utf-8")) > maximum or "\x00" in value:
        _fail("invalid_source_text")
    return value


def _native(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or len(value) > MAX_REACTOME_PATHWAYS:
        _fail("pathway_array_bound")
    result, identities, database_ids = [], set(), set()
    for row in value:
        if not isinstance(row, dict) or set(row)-_FIELDS or not _REQUIRED.issubset(row):
            _fail("native_pathway_fields")
        if row["schemaClass"] != "Pathway" or row["className"] != "Pathway" or row["speciesName"] != "Homo sapiens":
            _fail("native_pathway_scope")
        if type(row["dbId"]) is not int or not 1 <= row["dbId"] <= 10**12 or row["dbId"] in database_ids:
            _fail("native_database_identifier")
        database_ids.add(row["dbId"])
        stable = _text(row["stId"], maximum=64)
        if re.fullmatch(r"R-HSA-[1-9][0-9]{0,11}", stable) is None or stable in identities:
            _fail("native_stable_identifier")
        identities.add(stable)
        if type(row["stIdVersion"]) is not str or re.fullmatch(re.escape(stable)+r"\.[1-9][0-9]{0,8}", row["stIdVersion"]) is None:
            _fail("native_stable_version")
        _text(row["displayName"])
        for key in ("isInDisease", "isInferred", "hasDiagram", "hasEHLD"):
            if key in row and type(row[key]) is not bool:
                _fail("native_boolean_flag")
        if not isinstance(row["name"], list) or not 1 <= len(row["name"]) <= 64:
            _fail("native_name_array")
        for name in row["name"]:
            _text(name)
        if "maxDepth" in row and (type(row["maxDepth"]) is not int or not 0 <= row["maxDepth"] <= 128):
            _fail("native_depth_bound")
        for key in ("doi", "releaseStatus"):
            if key in row and row[key] is not None:
                _text(row[key], maximum=512)
        for key in ("releaseDate", "lastUpdatedDate"):
            if key in row and row[key] is not None:
                try:
                    if type(row[key]) is not str or datetime.strptime(row[key], "%Y-%m-%d").strftime("%Y-%m-%d") != row[key]:
                        _fail("native_date")
                except (ValueError, TypeError):
                    _fail("native_date")
        result.append(_safe(row))
    return result


_DEFAULT_BINDING = content_digest({"source": REACTOME_PATHWAY_BUNDLE_SCHEMA, "route": _URL,
    "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST})


def _bundle(config: ReactomeRetrievalConfig, *, retrieved_at: str, payload: Any,
            response_bytes: int, response_digest: str, mode: str, binding: str) -> dict[str, Any]:
    native = _native(payload)
    if type(response_bytes) is not int or not 1 <= response_bytes <= MAX_REACTOME_RESPONSE_BYTES or type(response_digest) is not str or _DIGEST.fullmatch(response_digest) is None:
        _fail("response_receipt_invalid")
    if mode not in {"default_reviewed_public_https", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_https" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    transport = _http_config(config)
    source = {"role": "human_pathway_mapping", "url": _URL, "http_status": 200,
        "response_bytes": response_bytes, "response_digest": response_digest,
        "native_payload": native, "payload_digest": content_digest(native)}
    rows = [{"stable_id": row["stId"], "stable_version": row["stIdVersion"],
        "display_name": row["displayName"], "species_name": row["speciesName"],
        "source_is_in_disease": row["isInDisease"], "source_is_inferred": row["isInferred"],
        "source_pointer": f"/{index}", "native_record": row} for index, row in enumerate(native)]
    receipt = {"schema": REACTOME_PATHWAY_RECEIPT_SCHEMA, "source_id": "reactome", "source_uri": _URL,
        "retrieved_at": retrieved_at, "response_bytes": response_bytes, "response_digest": response_digest,
        "payload_digest": source["payload_digest"], "source_request_count": 1,
        "returned_pathway_count": len(rows), "transport_mode": mode, "transport_binding_id": binding}
    value = {"schema": REACTOME_PATHWAY_BUNDLE_SCHEMA, "source_id": "reactome", "retrieved_at": retrieved_at,
        "query": {"resource": "UniProt", "identifier": "P04637", "species_taxon_id": 9606, "mapping_level": "lower_level_pathways", "timeout_ms": config.timeout_ms},
        "pathways": rows, "source_inputs": [source], "receipts": [receipt],
        "coverage": {"state": "source_annotations" if rows else "empty_success", "returned_pathway_count": len(rows),
            "reported_total_count": None, "exhaustiveness": "unassessed", "pagination": "not_documented",
            "hierarchy": "lower_level_mapping_only", "identity_validation": "source_assertions_only",
            "glioma_specificity": "not_established", "pathway_activity_measured": False},
        "transport": {"mode": mode, "binding_id": binding, "transport_policy_digest": transport.transport_policy_digest,
            "config_digest": transport.config_digest,
            "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_https" else "caller_owned",
            "automatic_retry": False}, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "bundle_digest": content_digest(value)})


def retrieve_reactome_pathways(config: ReactomeRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., ReactomeHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
        fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: ReactomeRetrievalConfig | None, *, approve_source_dispatch: bool,
        retrieved_at: str | None, fetcher: Callable[..., ReactomeHttpResponse] | None, binding: str) -> dict[str, Any]:
    config = ReactomeRetrievalConfig() if config is None else config
    if type(config) is not ReactomeRetrievalConfig:
        _fail("typed_config_required")
    if approve_source_dispatch is not True:
        _fail("source_approval_required")
    at = _clock(retrieved_at)
    http_config = _http_config(config)
    started = monotonic()
    try:
        if fetcher is None:
            response = ReviewedPublicHttpClient(http_config).get("tp53_human_pathways", approve_source_dispatch=True)
        else:
            if not callable(fetcher):
                _fail("fixture_fetcher_invalid")
            response = fetcher(_URL, http_config=http_config)
        if type(response) is not ReactomeHttpResponse:
            _fail("typed_response_required")
        # Re-admit the actual fields rather than trust a type whose frozen guard was bypassed.
        response = ReactomeHttpResponse(response.status, response.body, response.request_url,
            response.response_bytes, response.body_sha256, response.transport_policy_digest,
            response.config_digest, response.content_type)
        body, url, status = response.body, response.request_url, response.status
        if (response.config_digest != http_config.config_digest
                or response.transport_policy_digest != http_config.transport_policy_digest
                or response.content_type not in http_config.accepted_content_types):
            _fail("response_transport_metadata_mismatch")
    except (ReactomeRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise ReactomeRetrievalError("reactome_source_request_failed") from None
    if monotonic()-started > config.timeout_ms/1000:
        _fail("source_elapsed_deadline_exceeded")
    if url != _URL or type(status) is not int or status != 200:
        _fail("source_response_scope_or_status")
    payload = _parse(body)
    return _bundle(config, retrieved_at=at, payload=payload, response_bytes=len(body),
        response_digest=hashlib.sha256(body).hexdigest(),
        mode="default_reviewed_public_https" if fetcher is None else "caller_supplied", binding=binding)


def validate_reactome_pathway_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = ReactomeRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        source = actual["source_inputs"][0]
        expected = _bundle(config, retrieved_at=_clock(actual["retrieved_at"]), payload=source["native_payload"],
            response_bytes=source["response_bytes"], response_digest=source["response_digest"],
            mode=actual["transport"]["mode"], binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError):
        _fail("native_replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected):
        _fail("bundle_does_not_replay")
    return expected


def register_reactome_operations(catalog: Any, *, fetcher: Callable[..., ReactomeHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if not isinstance(catalog, WorkflowOperationCatalog) or (fetcher is not None and not callable(fetcher)):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(ReactomeRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": REACTOME_PATHWAY_OPERATION,
            "bundle_digest": bundle["bundle_digest"], "identity_validation": "source_assertions_only"})
    catalog.register(WorkflowOperation(REACTOME_PATHWAY_OPERATION,
        "Read fixed human UniProt P04637 lower-level Reactome pathway annotations; no glioma activity, enrichment, clinical or laboratory inference. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "pathways", "source_inputs", "coverage", "transport", "bundle_digest"],
         "properties": {"schema": {"const": REACTOME_PATHWAY_BUNDLE_SCHEMA}, "source_id": {"const": "reactome"},
            "query": {"type": "object"}, "pathways": {"type": "array", "maxItems": MAX_REACTOME_PATHWAYS},
            "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "coverage": {"type": "object"},
            "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "reactome", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog
