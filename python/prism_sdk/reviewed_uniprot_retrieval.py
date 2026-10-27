"""One bounded, reviewed human P04637 protein-name metadata request.

This repository-defined extension has no available blueprint identifier. Native names remain
untrusted source assertions. It requests no sequences, variants, features, structures, patient
records or clinical actions, and does not establish gene/protein identifier equivalence.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import math
import re
import secrets
import time
from typing import Any

from .authoring import canonical_json, content_digest
from .reviewed_public_http import (
    REVIEWED_PUBLIC_HTTP_POLICY_DIGEST, ReviewedPublicHttpClient, ReviewedPublicHttpConfig,
    ReviewedPublicHttpError, ReviewedPublicHttpResponse,
)
from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult


UNIPROT_ANNOTATION_BUNDLE_SCHEMA = "aurora-reviewed-uniprot-annotation/0.1"
UNIPROT_ANNOTATION_RECEIPT_SCHEMA = "aurora-reviewed-uniprot-annotation-receipt/0.1"
UNIPROT_OPERATION_ID = "uniprot.protein_annotation"
MAX_UNIPROT_RESPONSE_BYTES = 1_000_000
MAX_UNIPROT_TREE_DEPTH = 32
MAX_UNIPROT_TREE_NODES = 50_000
MAX_UNIPROT_NAME_ROWS = 256
MAX_UNIPROT_FIELD_OBSERVATIONS = 2_048
MAX_UNIPROT_LIST_ITEMS = 64
MAX_UNIPROT_GENE_RECORDS = 16
MAX_UNIPROT_NAME_BYTES = 2_048
_HOST = "rest.uniprot.org"
_ROUTE = "tp53_annotation"
_FIELDS = "accession,id,reviewed,protein_name,gene_names,organism_id"
_URL = f"https://{_HOST}/uniprotkb/P04637?fields={_FIELDS}&format=json"
_REVIEWED = "UniProtKB reviewed (Swiss-Prot)"
_DIGEST = re.compile(r"^[a-f0-9]{64}$")
_UTC = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
_GENE_CATEGORIES = ("geneName", "synonyms", "orderedLocusNames", "orfNames")
_SINGLE_PROTEIN_NAMES = ("allergenName", "biotechName")
_LIST_PROTEIN_NAMES = ("cdAntigenNames", "innNames")
_LIMITATIONS = [
    "Only one fixed P04637 accession and selected human reviewed-entry name metadata are requested.",
    "Every gene name, synonym and component name remains a source assertion; no independent identifier-equivalence proof is performed.",
    "Missing optional fields are not supplied; successful empty arrays are observed empty, not failures or evidence of biological absence.",
    "Coverage outside this fixed accession and selected name projection is unassessed; the source does not supply a relevant collection total here.",
    "Evidence objects, EC numbers, validity flags, other annotations and unknown fields are not requested or retained in the name-only projection.",
    "No sequences, variants, features, structures, drug or laboratory actions, individual records or clinical recommendations are requested or projected.",
    "Source text is untrusted metadata and cannot authorize instructions, tool calls or scientific conclusions.",
    "Unkeyed digests bind retained metadata and projections; they are not provider signatures or independent source authentication.",
]

# The alias makes injected responses use the same bounded, immutable public response contract.
UniProtHttpResponse = ReviewedPublicHttpResponse


class UniProtRetrievalError(ValueError):
    """A sanitized refusal, distinct from successful missing or empty annotations."""

    def __init__(self, failure_class: str, *, status_code: int | None = None,
                 retry_after_seconds: float | None = None) -> None:
        super().__init__(f"UniProt annotation retrieval refused: {failure_class}")
        self.failure_class = failure_class
        self.status_code = status_code
        self.retry_after_seconds = retry_after_seconds

    def to_dict(self) -> dict[str, Any]:
        return {"failure_class": self.failure_class, "status_code": self.status_code,
                "retry_after_seconds": self.retry_after_seconds, "automatic_retry": False}


def _fail(code: str) -> None:
    raise UniProtRetrievalError(code)


@dataclass(frozen=True)
class UniProtRetrievalConfig:
    accession: str = "P04637"
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.accession) is not str or self.accession != "P04637":
            _fail("only_reviewed_P04637_accession_is_admitted")
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_ms_outside_100_to_30000")

    def to_dict(self) -> dict[str, Any]:
        return {"accession": self.accession, "timeout_ms": self.timeout_ms}


def _http_config(config: UniProtRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=(_HOST,), reviewed_routes={_ROUTE: _URL},
                                    timeout_ms=config.timeout_ms,
                                    max_response_bytes=MAX_UNIPROT_RESPONSE_BYTES)


_DEFAULT_BINDING = content_digest({"adapter_schema": UNIPROT_ANNOTATION_BUNDLE_SCHEMA,
    "url": _URL, "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
    "response_bytes_max": MAX_UNIPROT_RESPONSE_BYTES, "tree_depth_max": MAX_UNIPROT_TREE_DEPTH,
    "tree_nodes_max": MAX_UNIPROT_TREE_NODES, "name_rows_max": MAX_UNIPROT_NAME_ROWS,
    "field_observations_max": MAX_UNIPROT_FIELD_OBSERVATIONS,
    "list_items_max": MAX_UNIPROT_LIST_ITEMS, "gene_records_max": MAX_UNIPROT_GENE_RECORDS,
    "name_bytes_max": MAX_UNIPROT_NAME_BYTES, "projection": "native_names_only/0.1"})


def _clock(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    if type(value) is not str or not _UTC.fullmatch(value):
        _fail("retrieved_at_must_be_utc_seconds")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        raise UniProtRetrievalError("retrieved_at_must_be_utc_seconds") from None
    return value


def _tree(value: Any) -> None:
    stack = [(value, 0)]
    nodes = 0
    while stack:
        node, depth = stack.pop()
        nodes += 1
        if nodes > MAX_UNIPROT_TREE_NODES or depth > MAX_UNIPROT_TREE_DEPTH:
            _fail("json_tree_bound")
        if isinstance(node, Mapping):
            if any(type(key) is not str for key in node):
                _fail("json_object_keys_must_be_strings")
            try:
                for key in node:
                    key.encode("utf-8")
            except UnicodeError:
                raise UniProtRetrievalError("json_string_invalid") from None
            stack.extend((child, depth + 1) for child in node.values())
        elif isinstance(node, list):
            stack.extend((child, depth + 1) for child in node)
        elif type(node) is str:
            try:
                node.encode("utf-8")
            except UnicodeError:
                raise UniProtRetrievalError("json_string_invalid") from None
        elif node is None or type(node) in {bool, int}:
            pass
        elif type(node) is float and math.isfinite(node):
            pass
        else:
            _fail("json_value_invalid")


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_UNIPROT_RESPONSE_BYTES:
        _fail("response_byte_bound")

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                _fail("duplicate_json_key")
            result[key] = value
        return result

    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=pairs,
                           parse_constant=lambda _value: _fail("nonfinite_json_number"))
        _tree(value)
    except UniProtRetrievalError:
        raise
    except (ValueError, UnicodeError, RecursionError):
        raise UniProtRetrievalError("response_json_invalid") from None
    if not isinstance(value, dict):
        _fail("source_entry_object_required")
    return value


def _object(value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        _fail("source_name_object_required")
    return value


def _list(value: Any, maximum: int = MAX_UNIPROT_LIST_ITEMS) -> list[Any]:
    if not isinstance(value, list) or len(value) > maximum:
        _fail("source_name_array_bound_or_type")
    return value


def _text(value: Any) -> str:
    if (type(value) is not str or not value.strip() or len(value.encode("utf-8")) > MAX_UNIPROT_NAME_BYTES
            or any(ord(char) < 32 or ord(char) == 127 for char in value)):
        _fail("source_name_text_invalid")
    return value


def _native_name(value: Any) -> dict[str, str]:
    value = _object(value)
    if "value" not in value:
        _fail("source_name_value_required")
    return {"value": _text(value["value"])}


def _protein_name(value: Any, *, short_names: bool = True) -> dict[str, Any]:
    source = _object(value)
    projected: dict[str, Any] = {}
    if "fullName" in source:
        projected["fullName"] = _native_name(source["fullName"])
    if short_names and "shortNames" in source:
        projected["shortNames"] = [_native_name(row) for row in _list(source["shortNames"])]
    return projected


def _protein_section(value: Any, *, top: bool) -> dict[str, Any]:
    source = _object(value)
    result: dict[str, Any] = {}
    if "recommendedName" in source:
        result["recommendedName"] = _protein_name(source["recommendedName"])
    if "alternativeNames" in source:
        result["alternativeNames"] = [_protein_name(row) for row in _list(source["alternativeNames"])]
    if top and "submissionNames" in source:
        result["submissionNames"] = [_protein_name(row, short_names=False)
                                     for row in _list(source["submissionNames"])]
    for key in _SINGLE_PROTEIN_NAMES:
        if key in source:
            result[key] = _native_name(source[key])
    for key in _LIST_PROTEIN_NAMES:
        if key in source:
            result[key] = [_native_name(row) for row in _list(source[key])]
    if top:
        for key in ("includes", "contains"):
            if key in source:
                result[key] = [_protein_section(row, top=False) for row in _list(source[key])]
    return result


def _project(source: Mapping[str, Any]) -> dict[str, Any]:
    if (type(source.get("primaryAccession")) is not str or source["primaryAccession"] != "P04637"
            or type(source.get("entryType")) is not str or source["entryType"] != _REVIEWED):
        _fail("fixed_reviewed_primary_accession_required")
    organism = _object(source.get("organism"))
    if type(organism.get("taxonId")) is not int or organism["taxonId"] != 9606:
        _fail("human_taxon_9606_required")
    result: dict[str, Any] = {"primaryAccession": "P04637", "entryType": _REVIEWED,
                              "organism": {"taxonId": 9606}}
    if "uniProtkbId" in source:
        result["uniProtkbId"] = _text(source["uniProtkbId"])
    if "proteinDescription" in source:
        result["proteinDescription"] = _protein_section(source["proteinDescription"], top=True)
    if "genes" in source:
        genes = []
        for row in _list(source["genes"], MAX_UNIPROT_GENE_RECORDS):
            row = _object(row)
            gene = {}
            if "geneName" in row:
                gene["geneName"] = _native_name(row["geneName"])
            for key in _GENE_CATEGORIES[1:]:
                if key in row:
                    gene[key] = [_native_name(name) for name in _list(row[key])]
            genes.append(gene)
        result["genes"] = genes
    return result


def _names(native: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    protein: list[dict[str, Any]] = []
    genes: list[dict[str, Any]] = []
    observations: list[dict[str, Any]] = []

    def observe(parent: Mapping[str, Any], key: str, path: str) -> None:
        if len(observations) >= MAX_UNIPROT_FIELD_OBSERVATIONS:
            _fail("field_observation_bound")
        present = key in parent
        value = parent.get(key)
        count = None if not present else (len(value) if isinstance(value, (list, dict)) else 1)
        observations.append({"source_pointer": path, "state": "not_supplied" if not present else
                             "empty_success" if count == 0 else "source_values", "returned_value_count": count})

    def add(value: Mapping[str, Any], path: str, category: str) -> None:
        protein.append({"category": category, "value": value["value"], "source_pointer": path + "/value"})

    def protein_name(value: Mapping[str, Any], path: str, category: str, *, short_names: bool = True) -> None:
        observe(value, "fullName", path + "/fullName")
        if "fullName" in value:
            add(value["fullName"], path + "/fullName", category + ".fullName")
        if short_names:
            observe(value, "shortNames", path + "/shortNames")
            for index, row in enumerate(value.get("shortNames", [])):
                add(row, path + f"/shortNames/{index}", category + ".shortNames")

    def section(value: Mapping[str, Any], path: str, prefix: str, *, top: bool) -> None:
        observe(value, "recommendedName", path + "/recommendedName")
        if "recommendedName" in value:
            protein_name(value["recommendedName"], path + "/recommendedName", prefix + "recommendedName")
        for key in (("alternativeNames", "submissionNames") if top else ("alternativeNames",)):
            observe(value, key, path + "/" + key)
            for index, row in enumerate(value.get(key, [])):
                protein_name(row, path + f"/{key}/{index}", prefix + key, short_names=key != "submissionNames")
        for key in _SINGLE_PROTEIN_NAMES:
            observe(value, key, path + "/" + key)
            if key in value:
                add(value[key], path + "/" + key, prefix + key)
        for key in _LIST_PROTEIN_NAMES:
            observe(value, key, path + "/" + key)
            for index, row in enumerate(value.get(key, [])):
                add(row, path + f"/{key}/{index}", prefix + key)
        if top:
            for key in ("includes", "contains"):
                observe(value, key, path + "/" + key)
                for index, row in enumerate(value.get(key, [])):
                    section(row, path + f"/{key}/{index}", key + ".", top=False)

    for key in ("uniProtkbId", "proteinDescription", "genes"):
        observe(native, key, "/" + key)
    if "proteinDescription" in native:
        section(native["proteinDescription"], "/proteinDescription", "", top=True)
    for index, row in enumerate(native.get("genes", [])):
        for key in _GENE_CATEGORIES:
            path = f"/genes/{index}/{key}"
            observe(row, key, path)
            values = [row[key]] if key == "geneName" and key in row else row.get(key, [])
            for name_index, name in enumerate(values):
                genes.append({"gene_index": index, "category": key, "value": name["value"],
                    "source_pointer": path + ("" if key == "geneName" else f"/{name_index}") + "/value"})
    if len(protein) + len(genes) > MAX_UNIPROT_NAME_ROWS:
        _fail("total_name_row_bound")
    return protein, genes, observations


def _transport(config: UniProtRetrievalConfig, mode: str, binding: str) -> dict[str, Any]:
    return {"mode": mode, "binding_id": binding,
            "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST,
            "config_digest": _http_config(config).config_digest,
            "timing_authority": "reviewed_public_http_total_deadline;adapter_checks_elapsed"
                if mode == "reviewed_public_https" else "caller_owned_preemption;adapter_checks_elapsed",
            "automatic_retry": False}


def _bundle(config: UniProtRetrievalConfig, stamp: str, source: dict[str, Any],
            transport: dict[str, Any]) -> dict[str, Any]:
    if set(source) != {"role", "url", "http_status", "response_bytes", "response_digest", "native_payload", "payload_digest"}:
        _fail("source_input_fields_invalid")
    if (source["role"] != "protein_annotation" or source["url"] != _URL
            or type(source["http_status"]) is not int or source["http_status"] != 200):
        _fail("source_input_route_or_status_invalid")
    if type(source["response_bytes"]) is not int or not 1 <= source["response_bytes"] <= MAX_UNIPROT_RESPONSE_BYTES:
        _fail("source_input_response_bound")
    if type(source["response_digest"]) is not str or not _DIGEST.fullmatch(source["response_digest"]):
        _fail("source_input_response_digest_invalid")
    if not isinstance(source["native_payload"], dict):
        _fail("retained_native_object_required")
    native = _project(source["native_payload"])
    if canonical_json(native) != canonical_json(source["native_payload"]):
        _fail("retained_native_fields_invalid")
    if source["payload_digest"] != content_digest(native):
        _fail("source_input_payload_digest_mismatch")
    protein, genes, observations = _names(native)
    state = {row["source_pointer"]: row["state"] for row in observations}
    receipt = {"schema": UNIPROT_ANNOTATION_RECEIPT_SCHEMA, "role": "protein_annotation", "source_uri": _URL,
        "request_digest": content_digest({"method": "GET", "url": _URL}),
        "response_digest": source["response_digest"], "response_bytes": source["response_bytes"],
        "payload_digest": source["payload_digest"], "http_status": 200, "retrieved_at": stamp,
        "transport_mode": transport["mode"], "transport_binding_id": transport["binding_id"],
        "transport_policy_digest": transport["transport_policy_digest"], "config_digest": transport["config_digest"]}
    result = {"schema": UNIPROT_ANNOTATION_BUNDLE_SCHEMA, "source_id": "uniprot", "retrieved_at": stamp,
        "query": config.to_dict(), "entry": {"requested_accession": "P04637", "primary_accession": "P04637",
            "entry_name": native.get("uniProtkbId"), "taxon_id": 9606, "entry_type": _REVIEWED, "reviewed": True},
        "protein_names": protein, "gene_name_candidates": genes, "field_observations": observations,
        "coverage": {"requested_accession_count": 1, "returned_entry_count": 1,
            "gene_records_state": state["/genes"], "gene_record_count": len(native["genes"]) if "genes" in native else None,
            "protein_description_state": state["/proteinDescription"],
            "protein_name_count": len(protein) if "proteinDescription" in native else None,
            "gene_name_count": len(genes) if "genes" in native else None,
            "reported_total_count": None, "exhaustiveness": "unassessed", "pagination": "not_applicable_single_entry",
            "identity_validation": "source_assertions_only", "source_request_count": 1,
            "response_byte_count": source["response_bytes"]},
        "source_inputs": [source], "receipts": [receipt], "transport": transport, "limitations": list(_LIMITATIONS)}
    return {**result, "bundle_digest": content_digest(result)}


def retrieve_uniprot_annotations(
    config: UniProtRetrievalConfig | None = None, *, approve_source_dispatch: bool = False,
    retrieved_at: str | None = None, fetcher: Callable[..., UniProtHttpResponse] | None = None,
) -> dict[str, Any]:
    """Fetch one exact credentialless entry after literal approval; no retries or pagination.

    An optional application-owned fetcher receives ``(fixed_url, *, http_config)``. It owns
    preemption and is always labelled caller_supplied, even if its bytes resemble a live entry.
    """
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
                     fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: UniProtRetrievalConfig | None, *, approve_source_dispatch: bool,
              retrieved_at: str | None, fetcher: Callable[..., UniProtHttpResponse] | None,
              binding: str) -> dict[str, Any]:
    config = UniProtRetrievalConfig() if config is None else config
    if not isinstance(config, UniProtRetrievalConfig):
        _fail("config_type_invalid")
    config = UniProtRetrievalConfig(**config.to_dict())
    if type(approve_source_dispatch) is not bool or approve_source_dispatch is not True:
        _fail("explicit_source_dispatch_approval_required")
    if fetcher is not None and not callable(fetcher):
        _fail("fetcher_must_be_application_owned_callable")
    stamp = _clock(retrieved_at)
    started = time.monotonic()
    http_config = _http_config(config)
    if time.monotonic() - started >= config.timeout_ms / 1000:
        _fail("total_deadline_exceeded")
    try:
        response = (ReviewedPublicHttpClient(http_config).get(_ROUTE, approve_source_dispatch=True)
                    if fetcher is None else fetcher(_URL, http_config=http_config))
    except ReviewedPublicHttpError as error:
        raise UniProtRetrievalError(error.failure_class, status_code=error.status_code,
                                   retry_after_seconds=error.retry_after_seconds) from None
    except Exception:
        raise UniProtRetrievalError("application_transport_failure") from None
    if time.monotonic() - started >= config.timeout_ms / 1000:
        _fail("total_deadline_exceeded")
    if (not isinstance(response, ReviewedPublicHttpResponse) or type(response.status) is not int or response.status != 200
            or response.request_url != _URL or response.content_type != "application/json"
            or response.config_digest != http_config.config_digest
            or response.transport_policy_digest != REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
            or type(response.body) is not bytes or type(response.response_bytes) is not int
            or not 1 <= len(response.body) <= MAX_UNIPROT_RESPONSE_BYTES
            or response.response_bytes != len(response.body)
            or response.body_sha256 != hashlib.sha256(response.body).hexdigest()):
        _fail("transport_response_contract_invalid")
    native = _project(_parse(response.body))
    source = {"role": "protein_annotation", "url": _URL, "http_status": 200,
              "response_bytes": response.response_bytes, "response_digest": response.body_sha256,
              "native_payload": native, "payload_digest": content_digest(native)}
    result = _bundle(config, stamp, source,
                     _transport(config, "reviewed_public_https" if fetcher is None else "caller_supplied", binding))
    if time.monotonic() - started >= config.timeout_ms / 1000:
        _fail("total_deadline_exceeded")
    return result


def validate_uniprot_annotation_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    """Defensively replay every retained name, omission state, pointer, receipt and digest.

    Original HTTP bytes are not retained. A consistently reauthored unkeyed artifact is not
    authenticated; this validation is internal integrity checking, with no source I/O.
    """
    if not isinstance(value, Mapping):
        _fail("bundle_object_required")
    try:
        _tree(value)
        encoded = canonical_json(dict(value))
        if len(encoded.encode("utf-8")) > 2 * MAX_UNIPROT_RESPONSE_BYTES:
            _fail("bundle_byte_bound")
        safe = json.loads(encoded)
        config = UniProtRetrievalConfig(**safe["query"])
        transport = safe["transport"]
        if (not isinstance(transport, dict) or transport.get("mode") not in {"reviewed_public_https", "caller_supplied"}
                or type(transport.get("binding_id")) is not str or not _DIGEST.fullmatch(transport["binding_id"])):
            _fail("bundle_transport_invalid")
        if transport["mode"] == "reviewed_public_https" and transport["binding_id"] != _DEFAULT_BINDING:
            _fail("default_transport_binding_mismatch")
        if canonical_json(transport) != canonical_json(_transport(config, transport["mode"], transport["binding_id"])):
            _fail("bundle_transport_posture_mismatch")
        sources = safe["source_inputs"]
        if not isinstance(sources, list) or len(sources) != 1:
            _fail("one_source_input_required")
        expected = _bundle(config, _clock(safe["retrieved_at"]), sources[0], transport)
        if canonical_json(safe) != canonical_json(expected):
            _fail("bundle_replay_mismatch")
        return expected
    except UniProtRetrievalError:
        raise
    except (KeyError, TypeError, ValueError, RecursionError, OverflowError):
        raise UniProtRetrievalError("bundle_shape_invalid") from None


def _obj(properties: dict[str, Any], *, optional: bool = False) -> dict[str, Any]:
    return {"type": "object", "properties": properties, "required": [] if optional else list(properties),
            "additionalProperties": False}


def _array(items: dict[str, Any], maximum: int, minimum: int = 0) -> dict[str, Any]:
    return {"type": "array", "items": items, "minItems": minimum, "maxItems": maximum}


def _output_schema() -> dict[str, Any]:
    text = {"type": "string"}
    digest = {"type": "string", "pattern": "^[a-f0-9]{64}$"}
    integer = {"type": "integer", "minimum": 0, "maximum": MAX_UNIPROT_RESPONSE_BYTES}
    nullable_count = {"anyOf": [integer, {"type": "null"}]}
    state = {"type": "string", "enum": ["not_supplied", "empty_success", "source_values"]}
    literal = lambda value: {"const": value, "type": "boolean" if type(value) is bool else
                             "integer" if type(value) is int else "string"}
    name = _obj({"value": {"type": "string", "minLength": 1, "maxLength": MAX_UNIPROT_NAME_BYTES}})
    protein_name = _obj({"fullName": name, "shortNames": _array(name, MAX_UNIPROT_LIST_ITEMS)}, optional=True)
    sub_name = _obj({"fullName": name}, optional=True)
    section_fields = {"recommendedName": protein_name, "alternativeNames": _array(protein_name, MAX_UNIPROT_LIST_ITEMS),
        **{key: name for key in _SINGLE_PROTEIN_NAMES},
        **{key: _array(name, MAX_UNIPROT_LIST_ITEMS) for key in _LIST_PROTEIN_NAMES}}
    section = _obj(section_fields, optional=True)
    protein_description = _obj({**section_fields, "submissionNames": _array(sub_name, MAX_UNIPROT_LIST_ITEMS),
        "includes": _array(section, MAX_UNIPROT_LIST_ITEMS), "contains": _array(section, MAX_UNIPROT_LIST_ITEMS)}, optional=True)
    gene = _obj({"geneName": name,
        **{key: _array(name, MAX_UNIPROT_LIST_ITEMS) for key in _GENE_CATEGORIES[1:]}}, optional=True)
    native = _obj({"primaryAccession": literal("P04637"), "entryType": literal(_REVIEWED),
        "organism": _obj({"taxonId": literal(9606)}), "uniProtkbId": text,
        "proteinDescription": protein_description, "genes": _array(gene, MAX_UNIPROT_GENE_RECORDS)}, optional=True)
    native["required"] = ["primaryAccession", "entryType", "organism"]
    source = _obj({"role": literal("protein_annotation"), "url": literal(_URL), "http_status": literal(200),
        "response_bytes": {"type": "integer", "minimum": 1, "maximum": MAX_UNIPROT_RESPONSE_BYTES},
        "response_digest": digest, "native_payload": native, "payload_digest": digest})
    mode = {"type": "string", "enum": ["reviewed_public_https", "caller_supplied"]}
    receipt = _obj({"schema": literal(UNIPROT_ANNOTATION_RECEIPT_SCHEMA), "role": literal("protein_annotation"),
        "source_uri": literal(_URL), "request_digest": digest, "response_digest": digest,
        "response_bytes": {"type": "integer", "minimum": 1, "maximum": MAX_UNIPROT_RESPONSE_BYTES},
        "payload_digest": digest, "http_status": literal(200), "retrieved_at": text,
        "transport_mode": mode, "transport_binding_id": digest, "transport_policy_digest": digest, "config_digest": digest})
    return _obj({"schema": literal(UNIPROT_ANNOTATION_BUNDLE_SCHEMA), "source_id": literal("uniprot"),
        "retrieved_at": text, "query": _obj({"accession": literal("P04637"), "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000}}),
        "entry": _obj({"requested_accession": literal("P04637"), "primary_accession": literal("P04637"),
            "entry_name": {"type": ["string", "null"]}, "taxon_id": literal(9606), "entry_type": literal(_REVIEWED), "reviewed": literal(True)}),
        "protein_names": _array(_obj({"category": text, "value": text, "source_pointer": text}), MAX_UNIPROT_NAME_ROWS),
        "gene_name_candidates": _array(_obj({"gene_index": {"type": "integer", "minimum": 0, "maximum": MAX_UNIPROT_GENE_RECORDS - 1},
            "category": {"type": "string", "enum": list(_GENE_CATEGORIES)}, "value": text, "source_pointer": text}), MAX_UNIPROT_NAME_ROWS),
        "field_observations": _array(_obj({"source_pointer": text, "state": state, "returned_value_count": nullable_count}), MAX_UNIPROT_FIELD_OBSERVATIONS),
        "coverage": _obj({"requested_accession_count": literal(1), "returned_entry_count": literal(1),
            "gene_records_state": state, "gene_record_count": nullable_count, "protein_description_state": state,
            "protein_name_count": nullable_count, "gene_name_count": nullable_count, "reported_total_count": {"type": "null"},
            "exhaustiveness": literal("unassessed"), "pagination": literal("not_applicable_single_entry"),
            "identity_validation": literal("source_assertions_only"), "source_request_count": literal(1), "response_byte_count": integer}),
        "source_inputs": _array(source, 1, 1), "receipts": _array(receipt, 1, 1),
        "transport": _obj({"mode": mode, "binding_id": digest, "transport_policy_digest": digest,
            "config_digest": digest, "timing_authority": text, "automatic_retry": literal(False)}),
        "limitations": _array(text, len(_LIMITATIONS), len(_LIMITATIONS)), "bundle_digest": digest})


def register_uniprot_operations(
    catalog: WorkflowOperationCatalog, *, fetcher: Callable[..., UniProtHttpResponse] | None = None,
) -> WorkflowOperationCatalog:
    """Opt in to one executable source operation; registration performs no request."""
    if not isinstance(catalog, WorkflowOperationCatalog):
        _fail("catalog_type_invalid")
    if fetcher is not None and not callable(fetcher):
        _fail("fetcher_must_be_application_owned_callable")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    mode = "reviewed_public_https" if fetcher is None else "caller_supplied"
    operation = WorkflowOperation(UNIPROT_OPERATION_ID,
        "Read one fixed P04637 human reviewed protein-name annotation. All aliases remain source assertions; "
        "no sequences, variants, clinical actions or scientific identifier join. "
        f"Transport mode: {mode}; binding: {binding}; policy: {REVIEWED_PUBLIC_HTTP_POLICY_DIGEST}.",
        {"type": "object", "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}},
         "additionalProperties": False}, _output_schema(), "uniprot", read_only=True,
        implementation="live_adapter" if fetcher is None else "caller_adapter")

    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> WorkflowOperationResult:
        bundle = _retrieve(UniProtRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
                           approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
                           fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {"schema": UNIPROT_ANNOTATION_RECEIPT_SCHEMA,
            "operation_id": UNIPROT_OPERATION_ID, "source_id": "uniprot", "bundle_digest": bundle["bundle_digest"],
            "source_request_count": 1, "response_bytes": bundle["coverage"]["response_byte_count"],
            "retrieved_at": bundle["retrieved_at"], "transport_mode": mode, "transport_binding_id": binding,
            "transport_policy_digest": bundle["transport"]["transport_policy_digest"],
            "config_digest": bundle["transport"]["config_digest"], "identity_validation": "source_assertions_only"})

    catalog.register(operation, execute)
    return catalog


__all__ = ["UNIPROT_ANNOTATION_BUNDLE_SCHEMA", "UNIPROT_ANNOTATION_RECEIPT_SCHEMA", "UNIPROT_OPERATION_ID",
           "UniProtRetrievalConfig", "UniProtHttpResponse", "UniProtRetrievalError", "retrieve_uniprot_annotations",
           "validate_uniprot_annotation_bundle", "register_uniprot_operations"]
