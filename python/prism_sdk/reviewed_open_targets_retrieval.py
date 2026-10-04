"""Reviewed, bounded Open Targets disease-to-gene association metadata retrieval.

The adapter queries one fixed glioma ontology entity per request and returns one explicitly
bounded top-ranked page. Association scores are source rankings, never confidence or clinical
evidence. The full response is transient; autonomous evidence receives digests and limitations.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass, field
from datetime import datetime, timezone
import json
import math
import re
from types import MappingProxyType
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration
from .errors import ArgumentError


REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA = "bioprism-reviewed-open-targets-config/0.1"
REVIEWED_OPEN_TARGETS_PLAN_SCHEMA = "bioprism-reviewed-open-targets-plan/0.1"
REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-open-targets-source-receipt/0.1"
REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA = "bioprism-reviewed-open-targets-bundle/0.1"
REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA = "bioprism-reviewed-open-targets-receipt/0.1"
REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA = "bioprism-reviewed-open-targets-transient/0.1"
REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-open-targets-execution-metadata/0.1"
REVIEWED_OPEN_TARGETS_ADAPTER_VERSION = "0.1"
REVIEWED_OPEN_TARGETS_HOST = "api.platform.opentargets.org"
REVIEWED_OPEN_TARGETS_ENDPOINT = f"https://{REVIEWED_OPEN_TARGETS_HOST}/api/v4/graphql"
REVIEWED_OPEN_TARGETS_AUTHORITY = "Open Targets Platform"
REVIEWED_OPEN_TARGETS_LANES = MappingProxyType({
    "gbm": "MONDO_0018177",
    "lgg": "MONDO_0021637",
})
MAX_REVIEWED_OPEN_TARGETS_LANES = 2
MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE = 50
MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES = 512_000
MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES = 1_024_000
MAX_REVIEWED_OPEN_TARGETS_BUNDLE_BYTES = 1_000_000
MAX_REVIEWED_OPEN_TARGETS_TREE_DEPTH = 20
MAX_REVIEWED_OPEN_TARGETS_TREE_NODES = 12_000
BUILTIN_OPEN_TARGETS_TRANSPORT_ID = "builtin.open-targets.urllib"
BUILTIN_OPEN_TARGETS_TRANSPORT_VERSION = "1"
_GRAPHQL_QUERY = (
    "query ReviewedDiseaseAssociations($efoId: String!, $pageIndex: Int!, $pageSize: Int!) "
    "{ disease(efoId: $efoId) { id name associatedTargets(page: { index: $pageIndex, size: $pageSize }) "
    "{ count rows { target { id approvedSymbol } score } } } }"
)
BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "urllib.request",
    "scheme": "https",
    "host": REVIEWED_OPEN_TARGETS_HOST,
    "path": "/api/v4/graphql",
    "method": "POST",
    "content_type": "application/json",
    "user_agent": "AURORA-Prism-SDK/0.1",
    "query_digest": content_digest(_GRAPHQL_QUERY),
    "variables": ["efoId", "pageIndex", "pageSize"],
    "page_index": 0,
    "redirects": "refused",
    "credentials": "not_accepted",
})

_RETENTION = "bounded_transient_target_ranking_metadata;autonomous_evidence_digest_only"
_LIMITATIONS = (
    "Open Targets association scores are ranking aids and are not confidence values",
    "disease association pages may include indirect ontology-propagated evidence",
    "only the first bounded source-ranked page is retrieved; omitted association rows remain explicit",
    "the fixed glioblastoma and low-grade glioma catalogue is not an exhaustive disease search",
    "source rankings and counts can change with Open Targets data releases",
    "independent review is required for evidence quality, freshness, omissions, and applicability",
    "the adapter does not retrieve patient-level, clinical outcome, or treatment-response data",
    "caller-injected transports own timeout, redirect, and network policy under their declared identity",
)
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_ENSEMBL_RE = re.compile(r"^ENSG[0-9]{11}$")
_SYMBOL_RE = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,63}$")
_LANE_TO_QUERY = {
    "gbm": ("MONDO_0018177", "glioblastoma"),
    "lgg": ("MONDO_0021637", "glioma"),
}


class ReviewedOpenTargetsRetrievalError(ArgumentError):
    """An Open Targets response or reviewed disease plan failed its bounded contract."""


OpenTargetsFetcher = Callable[[str, bytes, int], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedOpenTargetsRetrievalError(message)


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ReviewedOpenTargetsRetrievalError(f"{name} is not bounded JSON") from error
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


def _text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = " ".join(value.split())
    try:
        encoded = normalized.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ReviewedOpenTargetsRetrievalError(f"{name} contains invalid Unicode") from error
    if not normalized or len(encoded) > maximum or any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        _fail(f"{name} is empty or exceeds its text bound")
    return normalized


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(f"{name} must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedOpenTargetsRetrievalError(f"{name} is not a real UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _validate_tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_OPEN_TARGETS_TREE_NODES]
    budget[0] -= 1
    if budget[0] < 0 or depth > MAX_REVIEWED_OPEN_TARGETS_TREE_DEPTH:
        _fail("Open Targets response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("Open Targets response contains a non-text key")
            try:
                key.encode("utf-8")
            except UnicodeEncodeError as error:
                raise ReviewedOpenTargetsRetrievalError("Open Targets response contains invalid Unicode") from error
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, str):
        try:
            value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedOpenTargetsRetrievalError("Open Targets response contains invalid Unicode") from error
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("Open Targets response contains a non-finite number")
    elif value is not None and not isinstance(value, (str, int, float, bool)):
        _fail("Open Targets response contains an unsupported value")


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for key, value in pairs:
        if key in output:
            _fail("Open Targets response contains duplicate JSON fields")
        output[key] = value
    return output


def _parse_response(value: Any) -> tuple[Mapping[str, Any], int]:
    if isinstance(value, Mapping):
        encoded = _json_bytes(value, "Open Targets response", MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES)
        parsed: Any = value
    elif isinstance(value, str):
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedOpenTargetsRetrievalError("Open Targets response is not valid UTF-8") from error
        if len(encoded) > MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES:
            _fail("Open Targets response exceeds its byte bound")
        try:
            parsed = json.loads(encoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _value: _fail("Open Targets response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedOpenTargetsRetrievalError("Open Targets response is not valid JSON") from error
    elif isinstance(value, bytes):
        encoded = value
        if len(encoded) > MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES:
            _fail("Open Targets response exceeds its byte bound")
        try:
            parsed = json.loads(encoded.decode("utf-8"), object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _value: _fail("Open Targets response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedOpenTargetsRetrievalError("Open Targets response is not valid JSON") from error
    else:
        _fail("Open Targets response is not JSON")
    if not isinstance(parsed, Mapping):
        _fail("Open Targets response root is not an object")
    _validate_tree(parsed)
    return parsed, len(encoded)


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _builtin_fetch(body: bytes, timeout_ms: int) -> bytes:
    request = Request(
        REVIEWED_OPEN_TARGETS_ENDPOINT,
        data=body,
        headers={"Accept": "application/json", "Content-Type": "application/json", "User-Agent": "AURORA-Prism-SDK/0.1"},
        method="POST",
    )
    try:
        with build_opener(_NoRedirect).open(request, timeout=timeout_ms / 1000) as response:
            if response.geturl() != REVIEWED_OPEN_TARGETS_ENDPOINT or response.status != 200:
                _fail("Open Targets endpoint returned an unexpected response")
            result = response.read(MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError) as error:
        raise ReviewedOpenTargetsRetrievalError("Open Targets association request failed") from error
    if len(result) > MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES:
        _fail("Open Targets response exceeds its byte bound")
    return result


def _request_body(disease_id: str, page_size: int) -> bytes:
    body = _json_bytes({
        "query": _GRAPHQL_QUERY,
        "variables": {"efoId": disease_id, "pageIndex": 0, "pageSize": page_size},
    }, "Open Targets request", 8_192)
    if len(body) > 8_192:
        _fail("Open Targets request exceeds its byte bound")
    return body


def _association(raw: Any, lane: str, page_size: int) -> dict[str, Any]:
    if not isinstance(raw, Mapping) or set(raw) != {"data"}:
        _fail("Open Targets GraphQL response is malformed or contains errors")
    data = raw.get("data")
    if not isinstance(data, Mapping) or set(data) != {"disease"}:
        _fail("Open Targets response omitted the disease entity")
    disease = data.get("disease")
    disease_id, name_fragment = _LANE_TO_QUERY[lane]
    if not isinstance(disease, Mapping) or set(disease) != {"id", "name", "associatedTargets"} or disease.get("id") != disease_id:
        _fail("Open Targets disease identity differs from the reviewed lane")
    disease_name = _text("Open Targets disease name", disease.get("name"), 256)
    if name_fragment not in disease_name.casefold():
        _fail("Open Targets disease name does not match the reviewed lane")
    page = disease.get("associatedTargets")
    if not isinstance(page, Mapping) or set(page) != {"count", "rows"}:
        _fail("Open Targets response omitted association page metadata")
    total = _integer("Open Targets association count", page.get("count"), 0, 10_000_000)
    rows = page.get("rows")
    if not isinstance(rows, list) or len(rows) > page_size or len(rows) != min(total, page_size):
        _fail("Open Targets association page is incomplete or exceeds its reviewed bound")
    targets: list[dict[str, Any]] = []
    seen: set[str] = set()
    previous_score = 1.0
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping) or set(row) != {"target", "score"}:
            _fail("Open Targets association row has an unexpected shape")
        target = row.get("target")
        if not isinstance(target, Mapping) or set(target) != {"id", "approvedSymbol"}:
            _fail("Open Targets target identity is malformed")
        target_id = target.get("id")
        symbol = target.get("approvedSymbol")
        score = row.get("score")
        if not isinstance(target_id, str) or not _ENSEMBL_RE.fullmatch(target_id):
            _fail("Open Targets target identifier is not a stable Ensembl gene ID")
        if not isinstance(symbol, str) or not _SYMBOL_RE.fullmatch(symbol):
            _fail("Open Targets target symbol is invalid")
        if target_id in seen:
            _fail("Open Targets association page contains duplicate targets")
        if isinstance(score, bool) or not isinstance(score, (int, float)) or not math.isfinite(score) or not 0 <= score <= 1:
            _fail("Open Targets association score is outside its bounded range")
        if score > previous_score:
            _fail("Open Targets association rows are not ordered by descending source score")
        seen.add(target_id)
        previous_score = float(score)
        targets.append({"rank": index + 1, "target_id": target_id, "approved_symbol": symbol, "association_score": float(score)})
    return {
        "source_id": f"open_targets_{lane}",
        "lane": lane,
        "disease_id": disease_id,
        "disease_name": disease_name,
        "total_associations": total,
        "returned_associations": len(targets),
        "omitted_associations": total - len(targets),
        "page_size": page_size,
        "coverage": "all_source_rows" if total == len(targets) else "top_ranked_page_only",
        "score_interpretation": "ranking_only_not_confidence",
        "targets": targets,
    }


@dataclass(frozen=True, slots=True)
class ReviewedOpenTargetsRetrievalConfig:
    lanes: tuple[str, ...] = ("gbm",)
    page_size: int = 50
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_OPEN_TARGETS_TRANSPORT_ID
    transport_version: str = BUILTIN_OPEN_TARGETS_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST
    _lane_set_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.lanes, Sequence) or isinstance(self.lanes, (str, bytes)):
            _fail("Open Targets lanes must be a bounded list")
        lanes = tuple(self.lanes)
        if not lanes or len(lanes) > MAX_REVIEWED_OPEN_TARGETS_LANES or any(not isinstance(lane, str) or lane not in _LANE_TO_QUERY for lane in lanes) or len(set(lanes)) != len(lanes):
            _fail("Open Targets lanes contain an unsupported or duplicate lane")
        canonical_lanes = tuple(lane for lane in REVIEWED_OPEN_TARGETS_LANES if lane in lanes)
        object.__setattr__(self, "lanes", canonical_lanes)
        object.__setattr__(self, "page_size", _integer("Open Targets page_size", self.page_size, 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE))
        object.__setattr__(self, "timeout_ms", _integer("Open Targets timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER_RE.fullmatch(value):
                _fail(f"Open Targets {name} is invalid")
        _digest("Open Targets transport_config_digest", self.transport_config_digest)
        lane_digest = content_digest(self._lane_payload(canonical_lanes))
        if self._lane_set_digest and _digest("Open Targets lane_set_digest", self._lane_set_digest) != lane_digest:
            _fail("Open Targets lane_set_digest does not match the fixed disease catalogue")
        object.__setattr__(self, "_lane_set_digest", lane_digest)
        _json_bytes(self.to_dict(), "Open Targets config", 32_000)

    @staticmethod
    def _lane_payload(lanes: Sequence[str]) -> dict[str, Any]:
        return {"diseases": [{"lane": lane, "disease_id": _LANE_TO_QUERY[lane][0]} for lane in lanes]}

    @property
    def request_limit(self) -> int:
        return len(self.lanes)

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA,
            "lanes": list(self.lanes),
            "page_size": self.page_size,
            "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "lane_set_digest": self._lane_set_digest,
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedOpenTargetsRetrievalConfig":
        expected = {"schema", "lanes", "page_size", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "lane_set_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA or not isinstance(raw.get("lanes"), list):
            _fail("Open Targets config has an invalid shape")
        config = cls(tuple(raw["lanes"]), raw["page_size"], raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], raw["lane_set_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("Open Targets config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedOpenTargetsRetrievalPlan:
    config: ReviewedOpenTargetsRetrievalConfig
    config_digest: str
    lane_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedOpenTargetsRetrievalConfig) -> "ReviewedOpenTargetsRetrievalPlan":
        if type(config) is not ReviewedOpenTargetsRetrievalConfig:
            _fail("Open Targets plan requires an exact config")
        unsigned = {"schema": REVIEWED_OPEN_TARGETS_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "lane_set_digest": config._lane_set_digest, "request_limit": config.request_limit, "scope": "fixed_glioma_disease_top_ranked_association_pages", "execution": "bounded_https_graphql_post_per_lane_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(config, config.config_digest, config._lane_set_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {"schema": REVIEWED_OPEN_TARGETS_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "lane_set_digest": self.lane_set_digest, "request_limit": self.config.request_limit, "scope": "fixed_glioma_disease_top_ranked_association_pages", "execution": "bounded_https_graphql_post_per_lane_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if (self.config_digest, self.lane_set_digest, self.plan_digest) != (expected.config_digest, expected.lane_set_digest, expected.plan_digest):
            _fail("Open Targets plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedOpenTargetsRetrievalPlan":
        expected = {"schema", "config", "config_digest", "lane_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_OPEN_TARGETS_PLAN_SCHEMA:
            _fail("Open Targets plan has an invalid shape")
        plan = cls.create(ReviewedOpenTargetsRetrievalConfig.from_dict(raw["config"]))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("Open Targets plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedOpenTargetsRetrievalResult:
    _bundle: Mapping[str, Any] = field(repr=False)
    _receipt: Mapping[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(dict(self._bundle))

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(dict(self._receipt))

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": self.receipt, "retention": "digest_metadata_only"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_association_metadata"}


class ReviewedOpenTargetsRetrievalAdapter:
    """Read fixed, bounded target-disease association pages after explicit review."""

    def __init__(self, config: ReviewedOpenTargetsRetrievalConfig, *, fetch: OpenTargetsFetcher | None = None) -> None:
        if type(config) is not ReviewedOpenTargetsRetrievalConfig:
            _fail("Open Targets adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("Open Targets injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_OPEN_TARGETS_TRANSPORT_ID:
            _fail("Open Targets injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id != BUILTIN_OPEN_TARGETS_TRANSPORT_ID or config.transport_config_digest != BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST):
            _fail("Open Targets built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedOpenTargetsRetrievalPlan:
        return ReviewedOpenTargetsRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedOpenTargetsRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedOpenTargetsRetrievalResult:
        if type(plan) is not ReviewedOpenTargetsRetrievalPlan:
            _fail("Open Targets execution requires an exact reviewed plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("Open Targets execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("Open Targets dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "Open Targets retrieved_at")
        associations: list[dict[str, Any]] = []
        sources: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        response_bytes = 0
        for lane in self.config.lanes:
            disease_id = _LANE_TO_QUERY[lane][0]
            body = _request_body(disease_id, self.config.page_size)
            try:
                raw = _builtin_fetch(body, self.config.timeout_ms) if self._fetch is None else self._fetch(REVIEWED_OPEN_TARGETS_ENDPOINT, body, self.config.timeout_ms)
            except ReviewedOpenTargetsRetrievalError:
                raise
            except Exception as error:
                raise ReviewedOpenTargetsRetrievalError("Open Targets association request failed") from error
            response, consumed = _parse_response(raw)
            response_bytes += consumed
            if response_bytes > MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES:
                _fail("Open Targets aggregate response bytes exceed the plan bound")
            association = _association(response, lane, self.config.page_size)
            association_digest = content_digest(association)
            source_id = association["source_id"]
            sources.append({"source_id": source_id, "authority": REVIEWED_OPEN_TARGETS_AUTHORITY, "uri": REVIEWED_OPEN_TARGETS_ENDPOINT, "retrieved_at": timestamp, "content_sha256": association_digest, "record_count": association["returned_associations"], "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)})
            source_receipts.append({"schema": REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": source_id, "disease_id": disease_id, "content_digest": association_digest, "total_associations": association["total_associations"], "returned_associations": association["returned_associations"], "omitted_associations": association["omitted_associations"], "coverage": association["coverage"]})
            associations.append(association)
        coverage = "all_source_rows" if all(row["coverage"] == "all_source_rows" for row in associations) else "top_ranked_pages_only"
        source_set_digest = content_digest(sources)
        bundle_unsigned = {"schema": REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": sources, "associations": associations, "source_set_digest": source_set_digest, "association_count": len(associations), "coverage": coverage, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "Open Targets bundle", MAX_REVIEWED_OPEN_TARGETS_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        receipt_unsigned = {"schema": REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "lane_set_digest": plan.lane_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_set_digest, "source_count": len(sources), "association_count": len(associations), "request_count": len(associations), "response_bytes": response_bytes, "coverage": coverage, "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedOpenTargetsRetrievalResult(bundle, receipt)


def create_reviewed_open_targets_execution_metadata(plan: ReviewedOpenTargetsRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedOpenTargetsRetrievalPlan:
        _fail("Open Targets execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("Open Targets execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "Open Targets retrieved_at")
    payload = {"schema": REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedOpenTargetsRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    if not isinstance(value, Mapping) or set(value) != {"schema", "bundle", "receipt", "retention"} or value.get("schema") != REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA or value.get("retention") != "caller_owned_transient_association_metadata":
        _fail("Open Targets transient value is malformed")
    bundle, receipt = value.get("bundle"), value.get("receipt")
    if not isinstance(bundle, Mapping) or not isinstance(receipt, Mapping):
        _fail("Open Targets transient bundle or receipt is malformed")
    bundle_keys = {"schema", "generated_at", "sources", "associations", "source_set_digest", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_keys = {"schema", "plan_digest", "config_digest", "lane_set_digest", "bundle_digest", "source_set_digest", "source_count", "association_count", "request_count", "response_bytes", "coverage", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if set(bundle) != bundle_keys or set(receipt) != receipt_keys:
        _fail("Open Targets transient bundle or receipt has an unexpected shape")
    bundle_unsigned = {key: item for key, item in bundle.items() if key != "bundle_digest"}
    receipt_unsigned = {key: item for key, item in receipt.items() if key != "receipt_digest"}
    if content_digest(bundle_unsigned) != _digest("Open Targets bundle_digest", bundle.get("bundle_digest")) or content_digest(receipt_unsigned) != _digest("Open Targets receipt_digest", receipt.get("receipt_digest")):
        _fail("Open Targets transient digests are invalid")
    if bundle.get("schema") != REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA or receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("lane_set_digest") != plan.lane_set_digest:
        _fail("Open Targets transient identity differs from the reviewed plan")
    if receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("provider") != "none" or receipt.get("credentials") != "not_accepted" or receipt.get("retention") != _RETENTION or bundle.get("provider") != "none" or bundle.get("credentials") != "not_accepted":
        _fail("Open Targets transient boundary metadata is invalid")
    if canonical_json(bundle.get("limitations")) != canonical_json(_LIMITATIONS) or canonical_json(receipt.get("limitations")) != canonical_json(_LIMITATIONS):
        _fail("Open Targets transient limitations are invalid")
    retrieved_at = _timestamp(receipt.get("retrieved_at"), "Open Targets retrieved_at")
    if _timestamp(bundle.get("generated_at"), "Open Targets generated_at") != retrieved_at:
        _fail("Open Targets bundle and receipt timestamps do not match")
    associations, sources, source_receipts = bundle.get("associations"), bundle.get("sources"), receipt.get("source_receipts")
    if not isinstance(associations, list) or not isinstance(sources, list) or not isinstance(source_receipts, list) or len(associations) != len(plan.config.lanes) or len(sources) != len(associations) or len(source_receipts) != len(associations):
        _fail("Open Targets transient lane coverage is incomplete")
    if content_digest(sources) != bundle.get("source_set_digest") or receipt.get("source_set_digest") != bundle.get("source_set_digest") or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("Open Targets transient source binding is invalid")
    for index, lane in enumerate(plan.config.lanes):
        association = associations[index]
        if not isinstance(association, Mapping):
            _fail("Open Targets association record is malformed")
        if set(association) != {"source_id", "lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "page_size", "coverage", "score_interpretation", "targets"}:
            _fail("Open Targets association record has an unexpected shape")
        total = _integer("Open Targets association count", association.get("total_associations"), 0, 10_000_000)
        returned = _integer("Open Targets returned count", association.get("returned_associations"), 0, plan.config.page_size)
        omitted = _integer("Open Targets omitted count", association.get("omitted_associations"), 0, 10_000_000)
        targets = association.get("targets")
        if not isinstance(targets, list) or len(targets) != returned or returned != min(total, plan.config.page_size) or omitted != total - returned:
            _fail("Open Targets transient association counts are inconsistent")
        page_size = _integer("Open Targets page_size", association.get("page_size"), 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE)
        if association.get("lane") != lane or association.get("disease_id") != _LANE_TO_QUERY[lane][0] or _LANE_TO_QUERY[lane][1] not in _text("Open Targets disease name", association.get("disease_name"), 256).casefold() or association.get("source_id") != f"open_targets_{lane}" or page_size != plan.config.page_size or association.get("score_interpretation") != "ranking_only_not_confidence" or association.get("coverage") != ("all_source_rows" if total == returned else "top_ranked_page_only"):
            _fail("Open Targets association record differs from its reviewed lane")
        previous_score = 1.0
        seen: set[str] = set()
        for rank, target in enumerate(targets, 1):
            if not isinstance(target, Mapping) or set(target) != {"rank", "target_id", "approved_symbol", "association_score"} or _integer("Open Targets target rank", target.get("rank"), 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE) != rank:
                _fail("Open Targets target row identity is invalid")
            target_id, symbol, score = target.get("target_id"), target.get("approved_symbol"), target.get("association_score")
            if not isinstance(target_id, str) or not _ENSEMBL_RE.fullmatch(target_id) or target_id in seen or not isinstance(symbol, str) or not _SYMBOL_RE.fullmatch(symbol):
                _fail("Open Targets target row is invalid or duplicated")
            if isinstance(score, bool) or not isinstance(score, (int, float)) or not math.isfinite(score) or not 0 <= score <= previous_score:
                _fail("Open Targets target score ordering is invalid")
            previous_score = float(score)
            seen.add(target_id)
        expected_digest = content_digest(association)
        source, source_receipt = sources[index], source_receipts[index]
        if not isinstance(source, Mapping) or not isinstance(source_receipt, Mapping):
            _fail("Open Targets source metadata is malformed")
        expected_source = {"source_id": f"open_targets_{lane}", "authority": REVIEWED_OPEN_TARGETS_AUTHORITY, "uri": REVIEWED_OPEN_TARGETS_ENDPOINT, "retrieved_at": retrieved_at, "content_sha256": expected_digest, "record_count": returned, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        expected_receipt = {"schema": REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": f"open_targets_{lane}", "disease_id": _LANE_TO_QUERY[lane][0], "content_digest": expected_digest, "total_associations": total, "returned_associations": returned, "omitted_associations": omitted, "coverage": association["coverage"]}
        if canonical_json(source) != canonical_json(expected_source) or canonical_json(source_receipt) != canonical_json(expected_receipt):
            _fail("Open Targets source receipt does not match its association record")
    coverage = "all_source_rows" if all(row["coverage"] == "all_source_rows" for row in associations) else "top_ranked_pages_only"
    if _integer("Open Targets bundle association count", bundle.get("association_count"), 1, MAX_REVIEWED_OPEN_TARGETS_LANES) != len(associations) or _integer("Open Targets receipt association count", receipt.get("association_count"), 1, MAX_REVIEWED_OPEN_TARGETS_LANES) != len(associations) or _integer("Open Targets source count", receipt.get("source_count"), 1, MAX_REVIEWED_OPEN_TARGETS_LANES) != len(associations) or _integer("Open Targets request count", receipt.get("request_count"), 1, MAX_REVIEWED_OPEN_TARGETS_LANES) != len(associations) or bundle.get("coverage") != coverage or receipt.get("coverage") != coverage:
        _fail("Open Targets transient counts or coverage are inconsistent")
    _integer("Open Targets response_bytes", receipt.get("response_bytes"), len(associations), MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES)
    return bundle, receipt


def create_reviewed_open_targets_autonomous_evidence_registration(adapter: ReviewedOpenTargetsRetrievalAdapter, plan: ReviewedOpenTargetsRetrievalPlan, *, lane: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedOpenTargetsRetrievalAdapter or type(plan) is not ReviewedOpenTargetsRetrievalPlan:
        _fail("Open Targets registration requires exact adapter and plan values")
    plan.validate()
    if canonical_json(adapter.config.to_dict()) != canonical_json(plan.config.to_dict()) or lane not in plan.config.lanes or len(plan.config.lanes) != 1:
        _fail("Open Targets registration requires the exact single-lane plan")
    frozen_plan = ReviewedOpenTargetsRetrievalPlan.create(plan.config)
    source_id = f"open_targets_{lane}"
    expected_network = "builtin_https" if adapter.config.transport_id == BUILTIN_OPEN_TARGETS_TRANSPORT_ID else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("Open Targets acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected:
            _fail("Open Targets execution metadata has an invalid shape")
        unsigned = {key: value for key, value in metadata.items() if key != "metadata_digest"}
        if content_digest(unsigned) != _digest("Open Targets metadata digest", metadata.get("metadata_digest")) or metadata.get("schema") != REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("Open Targets execution metadata failed review binding")
        retrieved_at = None if metadata.get("retrieved_at") is None else _timestamp(metadata.get("retrieved_at"), "Open Targets retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=retrieved_at).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("Open Targets projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(
        adapter_id=f"reviewed.open_targets.{lane}",
        version=REVIEWED_OPEN_TARGETS_ADAPTER_VERSION,
        domains=("biomedical", "neuroscience"),
        capabilities=("target_disease_association_metadata", "source_provenance"),
        source_kinds=("open_targets_target_disease_association_metadata",),
        acquire=acquire,
        project=project,
    )


__all__ = [name for name in globals() if name.startswith("REVIEWED_OPEN_TARGETS_") or name.startswith("MAX_REVIEWED_OPEN_TARGETS_") or name.startswith("BUILTIN_OPEN_TARGETS_")] + [
    "OpenTargetsFetcher", "ReviewedOpenTargetsRetrievalError", "ReviewedOpenTargetsRetrievalConfig", "ReviewedOpenTargetsRetrievalPlan", "ReviewedOpenTargetsRetrievalResult", "ReviewedOpenTargetsRetrievalAdapter", "create_reviewed_open_targets_execution_metadata", "create_reviewed_open_targets_autonomous_evidence_registration",
]
