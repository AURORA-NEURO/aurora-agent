"""Source-native public GDC project SSM-data coverage, without an assay denominator.

Repository-defined application extension: no detailed blueprint module specifying this
public aggregate reader is available. The documented project-level availability count is
not gene-specific assay eligibility. It cannot authorize prevalence or clinical inference.
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

GDC_SSM_COVERAGE_BUNDLE_SCHEMA = "aurora-reviewed-gdc-ssm-coverage/0.1"
GDC_SSM_COVERAGE_RECEIPT_SCHEMA = "aurora-reviewed-gdc-ssm-coverage-receipt/0.1"
GDC_SSM_COVERAGE_OPERATION = "gdc.ssm_project_coverage"
GDC_SSM_COVERAGE_PROJECTS = ("TCGA-GBM", "TCGA-LGG")
MAX_GDC_SSM_COVERAGE_RESPONSE_BYTES = 1_000_000
MAX_GDC_SSM_COVERAGE_PROJECT_BUCKETS = 256
_URL = "https://api.gdc.cancer.gov/analysis/mutated_cases_count_by_project?size=0"
_ROUTE = "project_ssm_coverage"
_DIGEST = re.compile(r"[a-f0-9]{64}")
_LIMITATIONS = [
    "GDC case_with_ssm counts cases with associated SSM data under source semantics, not cases demonstrably assayed or callable for TP53.",
    "Project doc_count, nested case_summary doc_count and case_with_ssm doc_count are distinct native quantities; none is substituted for a gene-specific assay denominator.",
    "Only TCGA-GBM and TCGA-LGG are analyzed. All bounded public project buckets remain native inputs; other projects are explicitly unanalysed.",
    "Missing project/summary/SSM count fields remain unknown, distinct from an explicitly observed zero.",
    "The fixed aggregate response does not supply a gene, reference build, assay design, calling threshold or release identifier; compatibility with another acquisition is unestablished.",
    "Source terms-aggregation error/omission metadata is retained, without promoting exact bucket accounting to scientific coverage or gene callability.",
    "No individual hits, sample/case identifiers, mutation occurrences, sequences, raw variants, credentials or controlled-access records are requested or admitted.",
    "Digests and offline replay establish retained-byte consistency, not authenticated source truth, independent scientific review or clinical support.",
]


class GdcSsmCoverageRetrievalError(ArgumentError):
    """A bounded public aggregate request or retained artifact was not admissible."""


def _fail(code: str) -> None:
    raise GdcSsmCoverageRetrievalError("gdc_ssm_coverage_" + code)


@dataclass(frozen=True, slots=True)
class GdcSsmCoverageRetrievalConfig:
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_outside_bound")


GdcSsmCoverageHttpResponse = ReviewedPublicHttpResponse


def _http_config(config: GdcSsmCoverageRetrievalConfig) -> ReviewedPublicHttpConfig:
    return ReviewedPublicHttpConfig(allowed_hosts=("api.gdc.cancer.gov",), reviewed_routes={_ROUTE: _URL},
        timeout_ms=config.timeout_ms, max_response_bytes=MAX_GDC_SSM_COVERAGE_RESPONSE_BYTES)


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
    budget = [50_000] if budget is None else budget
    budget[0] -= 1
    if depth > 24 or budget[0] < 0:
        _fail("json_tree_bound")
    if isinstance(value, dict):
        for key, item in value.items():
            if type(key) is not str or len(key) > 128:
                _fail("json_key_bound")
            _tree(item, depth+1, budget)
    elif isinstance(value, list):
        for item in value:
            _tree(item, depth+1, budget)


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
    value = {}
    for key, item in pairs:
        if key in value:
            _fail("duplicate_json_field")
        value[key] = item
    return value


def _parse(body: bytes) -> dict[str, Any]:
    if type(body) is not bytes or not 1 <= len(body) <= MAX_GDC_SSM_COVERAGE_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        result = json.loads(body.decode("utf-8", "strict"), object_pairs_hook=_pairs,
            parse_constant=lambda _value: _fail("nonfinite_json"))
    except (UnicodeError, ValueError, RecursionError):
        _fail("response_json_invalid")
    return _safe(result, MAX_GDC_SSM_COVERAGE_RESPONSE_BYTES)


def _object(value: Any, required: set[str], optional: set[str] | None = None) -> dict[str, Any]:
    if type(value) is not dict or not required.issubset(value) or set(value)-required-(optional or set()):
        _fail("native_fields_changed")
    return value


def _count(value: Any, maximum: int = 10**12) -> int:
    if type(value) is not int or not 0 <= value <= maximum:
        _fail("native_count_invalid")
    return value


def _native(payload: Any) -> dict[str, Any]:
    root = _object(payload, {"took", "timed_out", "_shards", "hits", "aggregations"})
    _count(root["took"], 86_400_000)
    if root["timed_out"] is not False:
        _fail("source_timed_out")
    shards = _object(root["_shards"], {"total", "successful", "skipped", "failed"})
    for item in shards.values():
        _count(item, 10_000)
    if shards["total"] < 1 or shards["successful"] != shards["total"] or shards["failed"] != 0 or shards["skipped"] > shards["successful"]:
        _fail("source_shards_incomplete")
    hits = _object(root["hits"], {"total", "max_score", "hits"})
    total = _object(hits["total"], {"value", "relation"})
    _count(total["value"])
    if total["relation"] != "eq" or hits["max_score"] is not None or hits["hits"] != []:
        _fail("individual_hits_or_approximate_total")
    aggregate = _object(_object(root["aggregations"], {"projects"})["projects"],
        {"doc_count_error_upper_bound", "sum_other_doc_count", "buckets"})
    if any(type(aggregate[key]) is not int or aggregate[key] != 0 for key in ("doc_count_error_upper_bound", "sum_other_doc_count")):
        _fail("source_aggregate_incomplete")
    buckets = aggregate["buckets"]
    if type(buckets) is not list or len(buckets) > MAX_GDC_SSM_COVERAGE_PROJECT_BUCKETS:
        _fail("project_bucket_bound")
    seen = set()
    for row in buckets:
        _object(row, {"key", "doc_count"}, {"case_summary"})
        key = row["key"]
        if type(key) is not str or re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", key) is None or key in seen:
            _fail("project_identifier_invalid_or_duplicate")
        seen.add(key)
        _count(row["doc_count"])
        if "case_summary" in row:
            summary = _object(row["case_summary"], {"doc_count"}, {"case_with_ssm"})
            _count(summary["doc_count"])
            if "case_with_ssm" in summary:
                ssm = _object(summary["case_with_ssm"], set(), {"doc_count"})
                if "doc_count" in ssm:
                    count = _count(ssm["doc_count"])
                    if count > row["doc_count"] or count > summary["doc_count"]:
                        _fail("ssm_count_exceeds_native_parent")
    if sum(row["doc_count"] for row in buckets) > total["value"]:
        _fail("project_counts_exceed_exact_hit_total")
    return _safe(payload, MAX_GDC_SSM_COVERAGE_RESPONSE_BYTES)


def _projects(payload: dict[str, Any]) -> list[dict[str, Any]]:
    buckets = payload["aggregations"]["projects"]["buckets"]
    result = []
    for project in GDC_SSM_COVERAGE_PROJECTS:
        matches = [(index, row) for index, row in enumerate(buckets) if row["key"] == project]
        index, row = matches[0] if matches else (None, None)
        summary = None if row is None else row.get("case_summary")
        ssm = None if summary is None else summary.get("case_with_ssm")
        state = ("project_bucket_missing" if row is None else "case_summary_missing" if summary is None
            else "ssm_bucket_missing" if ssm is None else "ssm_count_missing" if "doc_count" not in ssm else "observed")
        path = None if row is None else f"/aggregations/projects/buckets/{index}"
        result.append({"project_id": project, "lane": "gbm" if project == "TCGA-GBM" else "lgg",
            "state": state, "project_doc_count": None if row is None else row["doc_count"],
            "case_summary_doc_count": None if summary is None else summary["doc_count"],
            "cases_with_ssm_data": None if state != "observed" else ssm["doc_count"],
            "native_project_pointer": path,
            "native_ssm_count_pointer": None if state != "observed" else path + "/case_summary/case_with_ssm/doc_count",
            "gene_specific_assay_denominator": None, "denominator_state": "not_established"})
    return result


_DEFAULT_BINDING = content_digest({"schema": GDC_SSM_COVERAGE_BUNDLE_SCHEMA, "url": _URL,
    "transport_policy_digest": REVIEWED_PUBLIC_HTTP_POLICY_DIGEST})


def _bundle(config: GdcSsmCoverageRetrievalConfig, *, body: bytes, retrieved_at: str,
            mode: str, binding: str) -> dict[str, Any]:
    payload = _native(_parse(body))
    if mode not in {"default_reviewed_public_https", "caller_supplied"} or type(binding) is not str or _DIGEST.fullmatch(binding) is None:
        _fail("transport_binding_invalid")
    if mode == "default_reviewed_public_https" and binding != _DEFAULT_BINDING:
        _fail("default_transport_binding_changed")
    http_config = _http_config(config)
    rows = _projects(payload)
    other = [row for row in payload["aggregations"]["projects"]["buckets"] if row["key"] not in GDC_SSM_COVERAGE_PROJECTS]
    raw_digest = hashlib.sha256(body).hexdigest()
    source = {"url": _URL, "http_status": 200, "response_body": body.decode("utf-8"),
        "response_bytes": len(body), "response_digest": raw_digest,
        "native_payload": payload, "payload_digest": content_digest(payload)}
    receipt = {"schema": GDC_SSM_COVERAGE_RECEIPT_SCHEMA, "source_id": "gdc",
        "source_uri": _URL, "retrieved_at": retrieved_at, "source_request_count": 1,
        "response_bytes": len(body), "response_digest": raw_digest,
        "transport_mode": mode, "transport_binding_id": binding}
    unsigned = {"schema": GDC_SSM_COVERAGE_BUNDLE_SCHEMA, "source_id": "gdc", "retrieved_at": retrieved_at,
        "query": {"route": "analysis/mutated_cases_count_by_project", "size": 0,
            "analyzed_projects": list(GDC_SSM_COVERAGE_PROJECTS), "gene_id": None, "timeout_ms": config.timeout_ms},
        "projects": rows, "source_inputs": [source], "receipts": [receipt],
        "coverage": {"observed_project_count": sum(row["state"] == "observed" for row in rows),
            "requested_project_count": 2, "returned_project_bucket_count": len(payload["aggregations"]["projects"]["buckets"]),
            "unanalysed_project_count": len(other), "unanalysed_project_digest": content_digest(other),
            "unanalysed_values_retained": True, "unanalysed_influence": "unknown" if other else "none_unanalysed",
            "source_terms_bucket_accounting": "source_reports_zero_error_and_omitted_documents",
            "scientific_completeness": "unassessed", "gene_assay_eligibility": "not_supplied",
            "source_release_id": None, "reference_build": None, "assay_definition": None,
            "prevalence_calculated": False, "clinical_actions_authorized": False},
        "transport": {"mode": mode, "binding_id": binding,
            "transport_policy_digest": http_config.transport_policy_digest, "config_digest": http_config.config_digest,
            "timing_authority": "builtin_total_deadline" if mode == "default_reviewed_public_https" else "caller_owned",
            "automatic_retry": False}, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "bundle_digest": content_digest(unsigned)})


def retrieve_gdc_ssm_coverage(config: GdcSsmCoverageRetrievalConfig | None = None, *,
        approve_source_dispatch: bool = False, retrieved_at: str | None = None,
        fetcher: Callable[..., GdcSsmCoverageHttpResponse] | None = None) -> dict[str, Any]:
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
        fetcher=fetcher, binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: GdcSsmCoverageRetrievalConfig | None, *, approve_source_dispatch: bool,
        retrieved_at: str | None, fetcher: Callable[..., GdcSsmCoverageHttpResponse] | None,
        binding: str) -> dict[str, Any]:
    config = GdcSsmCoverageRetrievalConfig() if config is None else config
    if type(config) is not GdcSsmCoverageRetrievalConfig:
        _fail("typed_config_required")
    config = GdcSsmCoverageRetrievalConfig(timeout_ms=config.timeout_ms)
    if approve_source_dispatch is not True:
        _fail("source_approval_required")
    at = _clock(retrieved_at)
    http_config = _http_config(config)
    expected_config = http_config.config_digest
    started = monotonic()
    try:
        if fetcher is None:
            response = ReviewedPublicHttpClient(http_config).get(_ROUTE, approve_source_dispatch=True)
        else:
            if not callable(fetcher):
                _fail("fetcher_invalid")
            response = fetcher(_URL, http_config=http_config)
        if type(response) is not GdcSsmCoverageHttpResponse:
            _fail("typed_response_required")
        response = GdcSsmCoverageHttpResponse(response.status, response.body, response.request_url,
            response.response_bytes, response.body_sha256, response.transport_policy_digest,
            response.config_digest, response.content_type)
        if (http_config.config_digest != expected_config or response.config_digest != expected_config
                or response.transport_policy_digest != REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
                or response.content_type != "application/json"):
            _fail("response_transport_metadata_changed")
    except (GdcSsmCoverageRetrievalError, ReviewedPublicHttpError):
        raise
    except Exception:
        raise GdcSsmCoverageRetrievalError("gdc_ssm_coverage_source_request_failed") from None
    if monotonic()-started > config.timeout_ms/1000:
        _fail("source_elapsed_deadline_exceeded")
    if response.request_url != _URL or type(response.status) is not int or response.status != 200:
        _fail("response_scope_or_status")
    return _bundle(config, body=response.body, retrieved_at=at,
        mode="default_reviewed_public_https" if fetcher is None else "caller_supplied", binding=binding)


def validate_gdc_ssm_coverage_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        config = GdcSsmCoverageRetrievalConfig(timeout_ms=actual["query"]["timeout_ms"])
        source = actual["source_inputs"][0]
        expected = _bundle(config, body=source["response_body"].encode("utf-8", "strict"),
            retrieved_at=_clock(actual["retrieved_at"]), mode=actual["transport"]["mode"],
            binding=actual["transport"]["binding_id"])
    except (KeyError, TypeError, IndexError, AttributeError, UnicodeError):
        _fail("replay_inputs_invalid")
    if canonical_json(actual) != canonical_json(expected):
        _fail("bundle_does_not_replay")
    return expected


def register_gdc_ssm_coverage_operations(catalog: Any, *,
        fetcher: Callable[..., GdcSsmCoverageHttpResponse] | None = None) -> Any:
    from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
    if not isinstance(catalog, WorkflowOperationCatalog) or (fetcher is not None and not callable(fetcher)):
        _fail("registration_arguments_invalid")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> Any:
        bundle = _retrieve(GdcSsmCoverageRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
            approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at, fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {**bundle["receipts"][0], "operation_id": GDC_SSM_COVERAGE_OPERATION,
            "bundle_digest": bundle["bundle_digest"], "gene_specific_assay_denominator": "not_established"})
    catalog.register(WorkflowOperation(GDC_SSM_COVERAGE_OPERATION,
        "Read fixed public GDC project SSM-data availability aggregates for TCGA-GBM/LGG; associated SSM data is not a TP53 assay denominator. Transport binding " + binding,
        {"type": "object", "additionalProperties": False, "properties": {"timeout_ms": {
            "type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}}},
        {"type": "object", "required": ["schema", "source_id", "query", "projects", "source_inputs", "coverage", "transport", "bundle_digest"],
            "properties": {"schema": {"const": GDC_SSM_COVERAGE_BUNDLE_SCHEMA}, "source_id": {"const": "gdc"},
                "query": {"type": "object"}, "projects": {"type": "array", "minItems": 2, "maxItems": 2},
                "source_inputs": {"type": "array", "minItems": 1, "maxItems": 1}, "coverage": {"type": "object"},
                "transport": {"type": "object"}, "bundle_digest": {"type": "string"}}},
        "gdc", read_only=True, implementation="live_adapter" if fetcher is None else "caller_adapter"), execute)
    return catalog
