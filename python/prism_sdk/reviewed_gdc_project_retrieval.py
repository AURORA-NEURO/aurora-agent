"""Reviewed, aggregate-only retrieval of fixed NCI GDC project metadata.

This adapter reads the public GDC project summary endpoint after literal caller approval. It
never requests cases, samples, files, molecular values, or controlled-access material. Its output
is a transient source artifact; autonomous evidence receives only digests and limitations.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from copy import deepcopy
from datetime import datetime, timezone
import json
import math
import re
from types import MappingProxyType
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration
from .errors import ArgumentError


REVIEWED_GDC_CONFIG_SCHEMA = "bioprism-reviewed-gdc-project-config/0.1"
REVIEWED_GDC_PLAN_SCHEMA = "bioprism-reviewed-gdc-project-plan/0.1"
REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-gdc-project-source-receipt/0.1"
REVIEWED_GDC_BUNDLE_SCHEMA = "bioprism-reviewed-gdc-project-bundle/0.1"
REVIEWED_GDC_RECEIPT_SCHEMA = "bioprism-reviewed-gdc-project-receipt/0.1"
REVIEWED_GDC_TRANSIENT_SCHEMA = "bioprism-reviewed-gdc-project-transient/0.1"
REVIEWED_GDC_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-gdc-project-execution-metadata/0.1"
REVIEWED_GDC_ADAPTER_VERSION = "0.1"
REVIEWED_GDC_HOST = "api.gdc.cancer.gov"
REVIEWED_GDC_PATH = "/projects"
REVIEWED_GDC_ENDPOINT = f"https://{REVIEWED_GDC_HOST}{REVIEWED_GDC_PATH}"
REVIEWED_GDC_AUTHORITY = "NCI Genomic Data Commons"
REVIEWED_GDC_PROJECTS = MappingProxyType({"gbm": "TCGA-GBM", "lgg": "TCGA-LGG"})
MAX_REVIEWED_GDC_PROJECTS = 2
MAX_REVIEWED_GDC_RESPONSE_BYTES = 1_000_000
MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES = 2_000_000
MAX_REVIEWED_GDC_BUNDLE_BYTES = 1_000_000
MAX_REVIEWED_GDC_TREE_DEPTH = 32
MAX_REVIEWED_GDC_TREE_NODES = 50_000
BUILTIN_GDC_TRANSPORT_ID = "builtin.nci-gdc.urllib"
BUILTIN_GDC_TRANSPORT_VERSION = "1"
BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "urllib.request",
    "scheme": "https",
    "host": REVIEWED_GDC_HOST,
    "path": REVIEWED_GDC_PATH,
    "method": "GET",
    "expand": ["summary", "summary.data_categories"],
    "fields": ["project_id", "name", "disease_type", "primary_site", "state", "released", "summary.case_count", "summary.file_count", "summary.data_categories"],
    "redirects": "refused",
    "credentials": "not_accepted",
})

_RETENTION = "aggregate_project_metadata_only;no_case_sample_or_file_rows"
_LIMITATIONS = (
    "GDC project summaries are aggregate source metadata, not patient-level evidence or outcome",
    "project and category counts are source-reported and may change between retrievals",
    "the fixed TCGA-GBM and TCGA-LGG catalogue is not an exhaustive glioma cohort search",
    "independent review is required for freshness, omissions, study quality, and applicability",
    "the adapter does not request or retain case, sample, file, sequence, assay, or controlled-access data",
    "caller-injected transports own timeout, redirect, and network policy under their declared identity",
)
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_PROJECT_TO_LANE = {project_id: lane for lane, project_id in REVIEWED_GDC_PROJECTS.items()}


class ReviewedGdcRetrievalError(ArgumentError):
    """A GDC response or reviewed project plan failed its bounded contract."""


GdcFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedGdcRetrievalError(message)


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ReviewedGdcRetrievalError(f"{name} is not bounded JSON") from error
    if len(encoded) > maximum:
        _fail(f"{name} exceeds its byte bound")
    return encoded


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        _fail(f"{name} is invalid")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its bounded integer range")
    return value


def _text(name: str, value: Any, maximum: int, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = " ".join(value.split())
    try:
        encoded = normalized.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ReviewedGdcRetrievalError(f"{name} contains invalid Unicode") from error
    if (not normalized and not optional) or len(encoded) > maximum or any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        _fail(f"{name} is empty or exceeds its text bound")
    return normalized or None


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(f"{name} must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedGdcRetrievalError(f"{name} is not a real UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _validate_tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_GDC_TREE_NODES]
    budget[0] -= 1
    if budget[0] < 0 or depth > MAX_REVIEWED_GDC_TREE_DEPTH:
        _fail("GDC response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("GDC response contains a non-text key")
            try:
                key.encode("utf-8")
            except UnicodeEncodeError as error:
                raise ReviewedGdcRetrievalError("GDC response contains invalid Unicode") from error
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, str):
        try:
            value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedGdcRetrievalError("GDC response contains invalid Unicode") from error
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("GDC response contains a non-finite number")
    elif value is not None and not isinstance(value, (str, int, float, bool)):
        _fail("GDC response contains an unsupported value")


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for key, value in pairs:
        if key in output:
            _fail("GDC response contains duplicate JSON fields")
        output[key] = value
    return output


def _parse_response(value: Any) -> tuple[Mapping[str, Any], int]:
    if isinstance(value, Mapping):
        encoded = _json_bytes(value, "GDC response", MAX_REVIEWED_GDC_RESPONSE_BYTES)
        parsed: Any = value
    elif isinstance(value, str):
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedGdcRetrievalError("GDC response is not valid UTF-8") from error
        if len(encoded) > MAX_REVIEWED_GDC_RESPONSE_BYTES:
            _fail("GDC response exceeds its byte bound")
        try:
            parsed = json.loads(encoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("GDC response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedGdcRetrievalError("GDC response is not valid JSON") from error
    elif isinstance(value, bytes):
        encoded = value
        if len(encoded) > MAX_REVIEWED_GDC_RESPONSE_BYTES:
            _fail("GDC response exceeds its byte bound")
        try:
            parsed = json.loads(encoded.decode("utf-8"), object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("GDC response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedGdcRetrievalError("GDC response is not valid JSON") from error
    else:
        _fail("GDC response is not JSON")
    if not isinstance(parsed, Mapping):
        _fail("GDC response root is not an object")
    _validate_tree(parsed)
    return parsed, len(encoded)


def _url(project_id: str) -> str:
    query = urlencode({
        "format": "json",
        "expand": "summary,summary.data_categories",
        "fields": "project_id,name,disease_type,primary_site,state,released,summary.case_count,summary.file_count,summary.data_categories",
    })
    return f"{REVIEWED_GDC_ENDPOINT}/{project_id}?{query}"


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    if not url.startswith(f"{REVIEWED_GDC_ENDPOINT}/"):
        _fail("GDC built-in transport refused an unpinned URL")
    request = Request(url, headers={"Accept": "application/json"}, method="GET")
    opener = build_opener(_NoRedirect)
    try:
        with opener.open(request, timeout=timeout_ms / 1000) as response:
            if response.geturl() != url or response.status != 200:
                _fail("GDC endpoint returned an unexpected response")
            body = response.read(MAX_REVIEWED_GDC_RESPONSE_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError) as error:
        raise ReviewedGdcRetrievalError("GDC project request failed") from error
    if len(body) > MAX_REVIEWED_GDC_RESPONSE_BYTES:
        _fail("GDC response exceeds its byte bound")
    return body


def _count(name: str, value: Any, *, optional: bool = False) -> int | None:
    if value is None and optional:
        return None
    return _integer(name, value, 0, 100_000_000)


def _text_list(name: str, value: Any) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list) or len(value) > 64:
        _fail(f"{name} exceeds its list bound")
    result = {_text(name, item, 512) for item in value}
    return sorted(item for item in result if item is not None)


def _project(raw: Any, project_id: str) -> dict[str, Any]:
    if not isinstance(raw, Mapping):
        _fail("GDC project response omitted data")
    data = raw.get("data")
    if not isinstance(data, Mapping) or data.get("project_id") != project_id:
        _fail("GDC project identity does not match the reviewed project")
    lane = _PROJECT_TO_LANE[project_id]
    name = _text("GDC project name", data.get("name"), 2_000)
    summary = data.get("summary")
    if not isinstance(summary, Mapping):
        _fail("GDC project response omitted summary metadata")
    case_count = _count("GDC case_count", summary.get("case_count"), optional=True)
    file_count = _count("GDC file_count", summary.get("file_count"), optional=True)
    raw_categories = summary.get("data_categories")
    if raw_categories is None:
        raw_categories = []
    if not isinstance(raw_categories, list) or len(raw_categories) > 256:
        _fail("GDC data categories exceed their list bound")
    categories: dict[str, dict[str, Any]] = {}
    for raw_category in raw_categories:
        if not isinstance(raw_category, Mapping):
            _fail("GDC data category is malformed")
        category = _text("GDC data category", raw_category.get("data_category"), 512)
        if category in categories:
            _fail("GDC project contains duplicate data categories")
        categories[category] = {
            "data_category": category,
            "case_count": _count("GDC category case_count", raw_category.get("case_count"), optional=True),
            "file_count": _count("GDC category file_count", raw_category.get("file_count"), optional=True),
        }
    raw_released = data.get("released")
    if raw_released is not None and not isinstance(raw_released, bool):
        _fail("GDC released must be a boolean")
    state = _text("GDC project state", data.get("state"), 128, optional=True)
    return {
        "source_id": f"gdc_{lane}",
        "lane": lane,
        "project_id": project_id,
        "name": name,
        "disease_types": _text_list("GDC disease_type", data.get("disease_type")),
        "primary_sites": _text_list("GDC primary_site", data.get("primary_site")),
        "state": state,
        "released": raw_released,
        "case_count": case_count,
        "file_count": file_count,
        "data_categories": [categories[key] for key in sorted(categories)],
        "metadata_completeness": "complete" if case_count is not None and file_count is not None and raw_released is not None else "unknown",
    }


@dataclass(frozen=True, slots=True)
class ReviewedGdcRetrievalConfig:
    project_ids: tuple[str, ...] = ("TCGA-GBM",)
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_GDC_TRANSPORT_ID
    transport_version: str = BUILTIN_GDC_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST
    _project_set_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.project_ids, Sequence) or isinstance(self.project_ids, (str, bytes)):
            _fail("GDC project_ids must be a bounded list")
        projects = tuple(self.project_ids)
        if not projects or len(projects) > MAX_REVIEWED_GDC_PROJECTS or any(not isinstance(project, str) or project not in _PROJECT_TO_LANE for project in projects) or len(set(projects)) != len(projects):
            _fail("GDC project_ids contain an unsupported or duplicate project")
        canonical_projects = tuple(project for project in REVIEWED_GDC_PROJECTS.values() if project in projects)
        object.__setattr__(self, "project_ids", canonical_projects)
        object.__setattr__(self, "timeout_ms", _integer("GDC timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER_RE.fullmatch(value):
                _fail(f"GDC {name} is invalid")
        _digest("GDC transport_config_digest", self.transport_config_digest)
        project_digest = content_digest(self._project_payload(canonical_projects))
        if self._project_set_digest and _digest("GDC project_set_digest", self._project_set_digest) != project_digest:
            _fail("GDC project_set_digest does not match the fixed project catalogue")
        object.__setattr__(self, "_project_set_digest", project_digest)
        _json_bytes(self.to_dict(), "GDC config", 32_000)

    @staticmethod
    def _project_payload(projects: Sequence[str]) -> dict[str, Any]:
        return {"projects": [{"lane": _PROJECT_TO_LANE[project], "project_id": project} for project in projects]}

    @property
    def request_limit(self) -> int:
        return len(self.project_ids)

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_GDC_CONFIG_SCHEMA,
            "project_ids": list(self.project_ids),
            "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "project_set_digest": self._project_set_digest,
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedGdcRetrievalConfig":
        expected = {"schema", "project_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "project_set_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_GDC_CONFIG_SCHEMA:
            _fail("GDC config has an invalid shape")
        config = cls(tuple(raw["project_ids"]), raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], raw["project_set_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("GDC config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedGdcRetrievalPlan:
    config: ReviewedGdcRetrievalConfig
    config_digest: str
    project_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedGdcRetrievalConfig) -> "ReviewedGdcRetrievalPlan":
        if type(config) is not ReviewedGdcRetrievalConfig:
            _fail("GDC plan requires an exact config")
        unsigned = {"schema": REVIEWED_GDC_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "project_set_digest": config._project_set_digest, "request_limit": config.request_limit, "scope": "fixed_public_gdc_project_aggregate_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(config, config.config_digest, config._project_set_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {"schema": REVIEWED_GDC_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "project_set_digest": self.project_set_digest, "request_limit": self.config.request_limit, "scope": "fixed_public_gdc_project_aggregate_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if (self.config_digest, self.project_set_digest, self.plan_digest) != (expected.config_digest, expected.project_set_digest, expected.plan_digest):
            _fail("GDC plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedGdcRetrievalPlan":
        expected = {"schema", "config", "config_digest", "project_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_GDC_PLAN_SCHEMA:
            _fail("GDC plan has an invalid shape")
        plan = cls.create(ReviewedGdcRetrievalConfig.from_dict(raw["config"]))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("GDC plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedGdcRetrievalResult:
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
        return {"schema": REVIEWED_GDC_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_project_metadata"}


class ReviewedGdcRetrievalAdapter:
    """Read only the fixed GDC project-summary endpoint after explicit review."""

    def __init__(self, config: ReviewedGdcRetrievalConfig, *, fetch: GdcFetcher | None = None) -> None:
        if type(config) is not ReviewedGdcRetrievalConfig:
            _fail("GDC adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("GDC injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_GDC_TRANSPORT_ID:
            _fail("GDC injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id != BUILTIN_GDC_TRANSPORT_ID or config.transport_config_digest != BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST):
            _fail("GDC built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedGdcRetrievalPlan:
        return ReviewedGdcRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedGdcRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedGdcRetrievalResult:
        if type(plan) is not ReviewedGdcRetrievalPlan:
            _fail("GDC execution requires an exact reviewed plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("GDC execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("GDC dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "GDC retrieved_at")
        projects: list[dict[str, Any]] = []
        sources: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        response_bytes = 0
        for project_id in self.config.project_ids:
            url = _url(project_id)
            try:
                raw = _builtin_fetch(url, self.config.timeout_ms) if self._fetch is None else self._fetch(url)
            except ReviewedGdcRetrievalError:
                raise
            except Exception as error:
                raise ReviewedGdcRetrievalError("GDC project request failed") from error
            response, consumed = _parse_response(raw)
            response_bytes += consumed
            if response_bytes > MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES:
                _fail("GDC aggregate response bytes exceed the plan bound")
            project = _project(response, project_id)
            project_digest = content_digest(project)
            source_id = project["source_id"]
            uri = f"{REVIEWED_GDC_ENDPOINT}/{project_id}"
            sources.append({"source_id": source_id, "authority": REVIEWED_GDC_AUTHORITY, "uri": uri, "retrieved_at": timestamp, "content_sha256": project_digest, "record_count": 1, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)})
            source_receipts.append({"schema": REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA, "lane": project["lane"], "source_id": source_id, "project_id": project_id, "content_digest": project_digest, "metadata_completeness": project["metadata_completeness"]})
            projects.append(project)
        completeness = "complete" if all(project["metadata_completeness"] == "complete" for project in projects) else "unknown"
        source_set_digest = content_digest(sources)
        bundle_unsigned = {"schema": REVIEWED_GDC_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": sources, "projects": projects, "source_set_digest": source_set_digest, "project_count": len(projects), "completeness": completeness, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "GDC bundle", MAX_REVIEWED_GDC_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        receipt_unsigned = {"schema": REVIEWED_GDC_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "project_set_digest": plan.project_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_set_digest, "source_count": len(sources), "project_count": len(projects), "request_count": len(projects), "response_bytes": response_bytes, "completeness": completeness, "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedGdcRetrievalResult(bundle, receipt)


def create_reviewed_gdc_execution_metadata(plan: ReviewedGdcRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedGdcRetrievalPlan:
        _fail("GDC execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("GDC execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "GDC retrieved_at")
    payload = {"schema": REVIEWED_GDC_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedGdcRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    if not isinstance(value, Mapping) or set(value) != {"schema", "bundle", "receipt", "retention"} or value.get("schema") != REVIEWED_GDC_TRANSIENT_SCHEMA or value.get("retention") != "caller_owned_transient_project_metadata":
        _fail("GDC transient value is malformed")
    bundle, receipt = value["bundle"], value["receipt"]
    if not isinstance(bundle, Mapping) or not isinstance(receipt, Mapping):
        _fail("GDC transient bundle or receipt is malformed")
    bundle_keys = {"schema", "generated_at", "sources", "projects", "source_set_digest", "project_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_keys = {"schema", "plan_digest", "config_digest", "project_set_digest", "bundle_digest", "source_set_digest", "source_count", "project_count", "request_count", "response_bytes", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if set(bundle) != bundle_keys or set(receipt) != receipt_keys:
        _fail("GDC transient bundle or receipt has an unexpected shape")
    bundle_unsigned = {key: item for key, item in bundle.items() if key != "bundle_digest"}
    receipt_unsigned = {key: item for key, item in receipt.items() if key != "receipt_digest"}
    if content_digest(bundle_unsigned) != _digest("GDC bundle_digest", bundle.get("bundle_digest")) or content_digest(receipt_unsigned) != _digest("GDC receipt_digest", receipt.get("receipt_digest")):
        _fail("GDC transient digests are invalid")
    if bundle.get("schema") != REVIEWED_GDC_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_GDC_RECEIPT_SCHEMA or receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("project_set_digest") != plan.project_set_digest:
        _fail("GDC transient identity differs from the reviewed plan")
    if receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("provider") != "none" or receipt.get("credentials") != "not_accepted" or receipt.get("retention") != _RETENTION or bundle.get("provider") != "none" or bundle.get("credentials") != "not_accepted":
        _fail("GDC transient boundary metadata is invalid")
    if canonical_json(bundle.get("limitations")) != canonical_json(list(_LIMITATIONS)) or canonical_json(receipt.get("limitations")) != canonical_json(list(_LIMITATIONS)):
        _fail("GDC transient limitations are invalid")
    timestamp = _timestamp(bundle.get("generated_at"), "GDC bundle timestamp")
    if timestamp != _timestamp(receipt.get("retrieved_at"), "GDC receipt timestamp"):
        _fail("GDC bundle and receipt timestamps do not match")
    projects, sources, source_receipts = bundle.get("projects"), bundle.get("sources"), receipt.get("source_receipts")
    if not all(isinstance(rows, list) for rows in (projects, sources, source_receipts)) or len(projects) != len(plan.config.project_ids) or len(sources) != len(projects) or len(source_receipts) != len(projects):
        _fail("GDC transient project coverage is incomplete")
    if content_digest(sources) != bundle.get("source_set_digest") or receipt.get("source_set_digest") != bundle.get("source_set_digest") or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("GDC transient source binding is invalid")
    for project_id, project, source, source_receipt in zip(plan.config.project_ids, projects, sources, source_receipts):
        source_keys = {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"}
        source_receipt_keys = {"schema", "lane", "source_id", "project_id", "content_digest", "metadata_completeness"}
        if not isinstance(project, Mapping) or not isinstance(source, Mapping) or not isinstance(source_receipt, Mapping) or set(source) != source_keys or set(source_receipt) != source_receipt_keys:
            _fail("GDC transient project source metadata is malformed")
        _integer("GDC source record_count", source.get("record_count"), 1, 1)
        normalized = _project({"data": {**project, "project_id": project_id, "disease_type": project.get("disease_types"), "primary_site": project.get("primary_sites"), "summary": {"case_count": project.get("case_count"), "file_count": project.get("file_count"), "data_categories": project.get("data_categories")}}}, project_id)
        project_digest = content_digest(normalized)
        if canonical_json(project) != canonical_json(normalized) or source.get("source_id") != normalized["source_id"] or source.get("authority") != REVIEWED_GDC_AUTHORITY or source.get("uri") != f"{REVIEWED_GDC_ENDPOINT}/{project_id}" or source.get("retrieved_at") != timestamp or source.get("content_sha256") != project_digest or source.get("provider") != "none" or source.get("credentials") != "not_accepted" or source.get("limitations") != list(_LIMITATIONS):
            _fail("GDC source metadata does not match its project")
        expected_receipt = {"schema": REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA, "lane": normalized["lane"], "source_id": normalized["source_id"], "project_id": project_id, "content_digest": project_digest, "metadata_completeness": normalized["metadata_completeness"]}
        if canonical_json(source_receipt) != canonical_json(expected_receipt):
            _fail("GDC source receipt does not match its project")
    for name, record, key in (("GDC request_count", receipt, "request_count"), ("GDC source_count", receipt, "source_count"), ("GDC receipt project_count", receipt, "project_count"), ("GDC bundle project_count", bundle, "project_count")):
        _integer(name, record.get(key), len(projects), len(projects))
    if receipt.get("request_count") != len(projects) or receipt.get("source_count") != len(projects) or receipt.get("project_count") != len(projects) or bundle.get("project_count") != len(projects):
        _fail("GDC transient counts are inconsistent")
    _integer("GDC response_bytes", receipt.get("response_bytes"), len(projects), MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES)
    expected_completeness = "complete" if all(project["metadata_completeness"] == "complete" for project in projects) else "unknown"
    if receipt.get("completeness") != expected_completeness or bundle.get("completeness") != expected_completeness:
        _fail("GDC transient completeness is inconsistent")
    return bundle, receipt


def create_reviewed_gdc_autonomous_evidence_registration(adapter: ReviewedGdcRetrievalAdapter, plan: ReviewedGdcRetrievalPlan, *, project_id: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedGdcRetrievalAdapter or type(plan) is not ReviewedGdcRetrievalPlan:
        _fail("GDC registration requires exact adapter and plan values")
    plan.validate()
    if not isinstance(project_id, str) or adapter.config != plan.config or project_id not in plan.config.project_ids or plan.config.project_ids != (project_id,):
        _fail("GDC registration requires the exact single-project plan")
    frozen_plan = ReviewedGdcRetrievalPlan.create(plan.config)
    lane = _PROJECT_TO_LANE[project_id]
    source_id = f"gdc_{lane}"
    expected_network = "builtin_https" if adapter._fetch is None else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("GDC acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected:
            _fail("GDC acquisition metadata is malformed")
        unsigned = dict(metadata)
        supplied = _digest("GDC metadata_digest", unsigned.pop("metadata_digest"))
        if supplied != content_digest(unsigned) or metadata.get("schema") != REVIEWED_GDC_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("GDC acquisition metadata failed review binding")
        retrieved_at = None if metadata["retrieved_at"] is None else _timestamp(metadata["retrieved_at"], "GDC retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=retrieved_at).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("GDC projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(adapter_id=f"reviewed.gdc.{lane}", version=REVIEWED_GDC_ADAPTER_VERSION, domains=("biomedical", "neuroscience"), capabilities=("aggregate_cohort_landscape", "source_provenance"), source_kinds=("nci_gdc_project_aggregate_metadata",), acquire=acquire, project=project)


__all__ = [name for name in globals() if name.startswith("REVIEWED_GDC_") or name.startswith("MAX_REVIEWED_GDC_") or name.startswith("BUILTIN_GDC_")] + [
    "GdcFetcher", "ReviewedGdcRetrievalError", "ReviewedGdcRetrievalConfig", "ReviewedGdcRetrievalPlan", "ReviewedGdcRetrievalResult", "ReviewedGdcRetrievalAdapter", "create_reviewed_gdc_execution_metadata", "create_reviewed_gdc_autonomous_evidence_registration",
]
