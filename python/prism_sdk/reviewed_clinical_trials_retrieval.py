"""Reviewed, bounded ClinicalTrials.gov study-metadata retrieval.

This adapter is a source-acquisition boundary, not an eligibility matcher or a clinical
recommendation system.  Plans use a fixed condition catalogue and exact field allow-list, request
only bounded pages from the public v2 studies endpoint, and require explicit dispatch approval.
Durable projections contain digests and counts; source records remain transient for the caller to
review and reconcile.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass, field
from datetime import date, datetime, timezone
import json
import math
import re
from types import MappingProxyType
from typing import Any
from urllib.parse import parse_qsl, urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .authoring import canonical_json, content_digest
from .autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistration


REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA = "bioprism-reviewed-clinical-trials-config/0.1"
REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA = "bioprism-reviewed-clinical-trials-plan/0.1"
REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-clinical-trials-source-receipt/0.1"
REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA = "bioprism-reviewed-clinical-trials-receipt/0.1"
REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA = "bioprism-reviewed-clinical-trials-bundle/0.1"
REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA = "bioprism-reviewed-clinical-trials-transient/0.1"
REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-clinical-trials-execution-metadata/0.1"
REVIEWED_CLINICAL_TRIALS_ADAPTER_VERSION = "0.1"
REVIEWED_CLINICAL_TRIALS_HOST = "clinicaltrials.gov"
REVIEWED_CLINICAL_TRIALS_PATH = "/api/v2/studies"
REVIEWED_CLINICAL_TRIALS_AUTHORITY = "ClinicalTrials.gov / U.S. National Library of Medicine"
REVIEWED_CLINICAL_TRIALS_FIELDS = (
    "NCTId", "BriefTitle", "OverallStatus", "Phase", "LastUpdatePostDate",
    "StudyType", "EnrollmentCount", "InterventionName",
)
REVIEWED_CLINICAL_TRIALS_CONDITIONS = MappingProxyType({"glioblastoma": "Glioblastoma", "glioma": "Glioma"})
MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE = 100
MAX_REVIEWED_CLINICAL_TRIALS_PAGES = 5
MAX_REVIEWED_CLINICAL_TRIALS_RECORDS = 2 * MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE * MAX_REVIEWED_CLINICAL_TRIALS_PAGES
MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES = 8_000_000
MAX_REVIEWED_CLINICAL_TRIALS_TOTAL_RESPONSE_BYTES = 24_000_000
MAX_REVIEWED_CLINICAL_TRIALS_BUNDLE_BYTES = 8_000_000
MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH = 64
MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES = 200_000
BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID = "builtin.clinicaltrials.gov.https-get"
BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION = "1"
BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "allowlisted_https_get_v1",
    "method": "GET",
    "scheme": "https",
    "host": REVIEWED_CLINICAL_TRIALS_HOST,
    "path": REVIEWED_CLINICAL_TRIALS_PATH,
    "redirects": "refused",
    "fields": list(REVIEWED_CLINICAL_TRIALS_FIELDS),
    "credentials": "not_accepted",
})

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_NCT = re.compile(r"^NCT[0-9]{8}$")
_DATE = re.compile(r"^[0-9]{4}-[0-9]{2}(?:-[0-9]{2})?$")
_TOKEN = re.compile(r"^[!-~]{1,512}$")
_TEXT_WHITESPACE = re.compile(r"[\t\n\v\f\r \u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000\ufeff]+")
_SOURCE_BASE = "https://clinicaltrials.gov/api/v2/studies"
_RETENTION = "metadata_only;trial_records_and_page_tokens_transient"
_LIMITATIONS = (
    "ClinicalTrials.gov registry metadata is not evidence of eligibility, treatment benefit, or clinical applicability",
    "a bounded page sequence can be incomplete; the receipt reports truncation and source-reported totals",
    "the adapter does not retrieve participant-level data, eligibility text, results, documents, or contact fields",
    "source records require independent review before any research conclusion or workflow promotion",
    "caller-injected transports must enforce their own timeout and network policy under the supplied transport identity",
)


class ReviewedClinicalTrialsRetrievalError(ValueError):
    """A reviewed ClinicalTrials.gov request or response violated its exact bounded contract."""


# Injected transports own their timeout and network policy under the configured identity.
ClinicalTrialsFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedClinicalTrialsRetrievalError(message)


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} must be an integer between {minimum} and {maximum}")
    return value


def _text(name: str, value: Any, maximum: int, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str):
        _fail(f"{name} must be text")
    normalized = _TEXT_WHITESPACE.sub(" ", value).strip(" ")
    if not normalized and optional:
        return None
    try:
        encoded_length = len(normalized.encode("utf-8", errors="strict"))
    except UnicodeEncodeError as error:
        raise ReviewedClinicalTrialsRetrievalError(f"{name} contains invalid Unicode") from error
    if not normalized or "\x00" in normalized or encoded_length > maximum:
        _fail(f"{name} is outside its text bound")
    if any((ord(character) < 32 and character not in "\t\n\r") or ord(character) == 127 for character in normalized):
        _fail(f"{name} contains a control character")
    return normalized


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", value):
        _fail(f"{name} must be a UTC timestamp with second precision")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedClinicalTrialsRetrievalError(f"{name} is not a valid UTC timestamp") from error
    return value


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _canonical_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError, RecursionError) as error:
        raise ReviewedClinicalTrialsRetrievalError(f"{name} is not canonical JSON") from error
    if len(encoded) > maximum:
        _fail(f"{name} exceeds its byte bound")
    return encoded


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail("ClinicalTrials.gov response contains a duplicate JSON field")
        result[key] = value
    return result


def _validate_response_tree(value: Any) -> None:
    stack = [(value, 0)]
    seen: set[int] = set()
    nodes = 0
    while stack:
        current, depth = stack.pop()
        nodes += 1
        if nodes > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES or depth > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH:
            _fail("ClinicalTrials.gov response exceeds its tree bound")
        if isinstance(current, Mapping):
            if not isinstance(current, dict):
                _fail("ClinicalTrials.gov response contains a non-JSON object")
            identity = id(current)
            if identity in seen:
                _fail("ClinicalTrials.gov response is not a JSON tree")
            seen.add(identity)
            for key, child in current.items():
                if not isinstance(key, str):
                    _fail("ClinicalTrials.gov response contains a non-text object key")
                stack.append((child, depth + 1))
        elif isinstance(current, list):
            identity = id(current)
            if identity in seen:
                _fail("ClinicalTrials.gov response is not a JSON tree")
            seen.add(identity)
            stack.extend((child, depth + 1) for child in current)
        elif isinstance(current, float) and not math.isfinite(current):
            _fail("ClinicalTrials.gov response contains a non-finite number")
        elif current is None or isinstance(current, (str, bool, int, float)):
            continue
        else:
            _fail("ClinicalTrials.gov response contains a non-JSON value")


def _decode_response(value: Any) -> Mapping[str, Any]:
    if isinstance(value, (bytes, str)):
        raw = value if isinstance(value, bytes) else value.encode("utf-8")
        if len(raw) > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES:
            _fail("ClinicalTrials.gov response exceeds its byte bound")
        try:
            decoded = raw.decode("utf-8", errors="strict")
            payload = json.loads(decoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("ClinicalTrials.gov response contains a non-finite number"))
        except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
            raise ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov response is not valid UTF-8 JSON") from error
    elif isinstance(value, Mapping):
        payload = dict(value)
        _validate_response_tree(payload)
        _canonical_bytes(payload, "ClinicalTrials.gov response", MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES)
    else:
        _fail("ClinicalTrials.gov transport returned an unsupported response type")
    if not isinstance(payload, Mapping):
        _fail("ClinicalTrials.gov response root is not an object")
    _validate_response_tree(payload)
    return payload


def _url(condition: str, page_size: int, page_token: str | None) -> str:
    parameters: list[tuple[str, str]] = [
        ("format", "json"),
        ("fields", ",".join(REVIEWED_CLINICAL_TRIALS_FIELDS)),
        ("pageSize", str(page_size)),
        ("query.cond", condition),
    ]
    if page_token is not None:
        parameters.append(("pageToken", page_token))
    return f"{_SOURCE_BASE}?{urlencode(parameters)}"


def _validate_url(url: str, condition: str, page_size: int, page_token: str | None) -> None:
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.hostname != REVIEWED_CLINICAL_TRIALS_HOST or parsed.path != REVIEWED_CLINICAL_TRIALS_PATH or parsed.username or parsed.password or parsed.fragment:
        _fail("ClinicalTrials.gov request escaped its exact HTTPS authority")
    expected = [("format", "json"), ("fields", ",".join(REVIEWED_CLINICAL_TRIALS_FIELDS)), ("pageSize", str(page_size)), ("query.cond", condition)]
    if page_token is not None:
        expected.append(("pageToken", page_token))
    if parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True) != expected:
        _fail("ClinicalTrials.gov request parameters differ from the reviewed query")


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, _request: Request, _fp: Any, _code: int, _message: str, _headers: Any, _new_url: str) -> None:
        return None


def _builtin_fetch(url: str, timeout: float) -> bytes:
    try:
        with build_opener(_NoRedirect()).open(Request(url, headers={"Accept": "application/json"}), timeout=timeout) as response:
            if response.status != 200:
                _fail("ClinicalTrials.gov returned a non-success status")
            body = response.read(MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES + 1)
    except ReviewedClinicalTrialsRetrievalError:
        raise
    except Exception as error:
        raise ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov request failed") from error
    if len(body) > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES:
        _fail("ClinicalTrials.gov response exceeds its byte bound")
    return body


def _path(value: Mapping[str, Any], *keys: str) -> Any:
    current: Any = value
    for key in keys:
        if not isinstance(current, Mapping):
            return None
        current = current.get(key)
    return current


def _list_text(value: Any, name: str, maximum: int) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list) or len(value) > maximum:
        _fail(f"{name} is outside its list bound")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        if item is None or item == "":
            continue
        text = _text(name, item, 1024)
        assert text is not None
        if text not in seen:
            seen.add(text)
            result.append(text)
    return result


def _trial(study: Any, source_id: str) -> dict[str, Any]:
    if not isinstance(study, Mapping):
        _fail("ClinicalTrials.gov study entry is not an object")
    protocol = study.get("protocolSection")
    if not isinstance(protocol, Mapping):
        _fail("ClinicalTrials.gov study omitted protocolSection")
    nct_id = _text("ClinicalTrials.gov NCT ID", _path(protocol, "identificationModule", "nctId"), 16)
    if nct_id is None or not _NCT.fullmatch(nct_id):
        _fail("ClinicalTrials.gov study has an invalid NCT identifier")
    title = _text("ClinicalTrials.gov brief title", _path(protocol, "identificationModule", "briefTitle"), 4096)
    status = _text("ClinicalTrials.gov overall status", _path(protocol, "statusModule", "overallStatus"), 128)
    design = _path(protocol, "designModule")
    if not isinstance(design, Mapping):
        design = {}
    phases = _list_text(design.get("phases"), "ClinicalTrials.gov phases", 16)
    last_update = _text("ClinicalTrials.gov last update", _path(protocol, "statusModule", "lastUpdatePostDateStruct", "date"), 10, optional=True)
    if last_update is not None:
        if not _DATE.fullmatch(last_update):
            _fail("ClinicalTrials.gov last update is outside the source date contract")
        try:
            if len(last_update) == 7:
                datetime.strptime(last_update, "%Y-%m")
            else:
                date.fromisoformat(last_update)
        except ValueError as error:
            raise ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov last update is not a valid calendar date") from error
    study_type = _text("ClinicalTrials.gov study type", design.get("studyType"), 128, optional=True)
    enrollment = _path(design, "enrollmentInfo", "count")
    if enrollment is not None:
        enrollment = _integer("ClinicalTrials.gov enrollment target", enrollment, 0, 10_000_000)
    interventions = _path(protocol, "armsInterventionsModule", "interventions")
    if interventions is None:
        interventions = []
    if not isinstance(interventions, list) or len(interventions) > 128:
        _fail("ClinicalTrials.gov interventions are outside their list bound")
    names = _list_text([_path(row, "name") for row in interventions if isinstance(row, Mapping)], "ClinicalTrials.gov intervention name", 128)
    return {
        "source_id": source_id,
        "nct_id": nct_id,
        "title": title,
        "overall_status": status,
        "phases": phases,
        "last_update": last_update,
        "study_type": study_type,
        "enrollment_count": enrollment,
        "intervention_names": names,
    }


def _bundle_digest(bundle: Mapping[str, Any]) -> str:
    unsigned = dict(bundle)
    unsigned.pop("bundle_digest", None)
    return content_digest(unsigned)


@dataclass(frozen=True, slots=True)
class ReviewedClinicalTrialsRetrievalConfig:
    condition_lanes: tuple[str, ...] = ("glioblastoma",)
    page_size: int = 25
    max_pages: int = 2
    timeout_seconds: int = 30
    transport_id: str = BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID
    transport_version: str = BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST

    def __post_init__(self) -> None:
        if isinstance(self.condition_lanes, (str, bytes)) or not isinstance(self.condition_lanes, Sequence) or not 1 <= len(self.condition_lanes) <= len(REVIEWED_CLINICAL_TRIALS_CONDITIONS):
            _fail("condition_lanes must select one or more fixed glioma lanes")
        lanes = tuple(self.condition_lanes)
        if any(not isinstance(lane, str) or lane not in REVIEWED_CLINICAL_TRIALS_CONDITIONS for lane in lanes) or len(set(lanes)) != len(lanes):
            _fail("condition_lanes contains an unsupported or duplicate lane")
        object.__setattr__(self, "condition_lanes", tuple(sorted(lanes)))
        _integer("page_size", self.page_size, 1, MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE)
        _integer("max_pages", self.max_pages, 1, MAX_REVIEWED_CLINICAL_TRIALS_PAGES)
        _integer("timeout_seconds", self.timeout_seconds, 1, 120)
        for name, value in (("transport_id", self.transport_id), ("transport_version", self.transport_version)):
            if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_.:+-]{1,128}", value):
                _fail(f"{name} is outside its identifier contract")
        _digest("transport_config_digest", self.transport_config_digest)
        _canonical_bytes(self.to_dict(), "ClinicalTrials.gov config", 64_000)

    @property
    def request_limit(self) -> int:
        return len(self.condition_lanes) * self.max_pages

    @property
    def record_limit(self) -> int:
        return self.request_limit * self.page_size

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA,
            "condition_lanes": list(self.condition_lanes),
            "page_size": self.page_size,
            "max_pages": self.max_pages,
            "request_limit": self.request_limit,
            "record_limit": self.record_limit,
            "timeout_seconds": self.timeout_seconds,
            "fields": list(REVIEWED_CLINICAL_TRIALS_FIELDS),
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "retention": "metadata_only;source_values_transient",
            "credentials": "not_accepted",
        }

    @property
    def config_digest(self) -> str:
        return content_digest(self.to_dict())


@dataclass(frozen=True, slots=True)
class ReviewedClinicalTrialsRetrievalPlan:
    config: ReviewedClinicalTrialsRetrievalConfig
    config_digest: str
    query_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedClinicalTrialsRetrievalConfig) -> "ReviewedClinicalTrialsRetrievalPlan":
        if type(config) is not ReviewedClinicalTrialsRetrievalConfig:
            _fail("ClinicalTrials.gov plan requires an exact reviewed config")
        queries = [{"lane": lane, "condition": REVIEWED_CLINICAL_TRIALS_CONDITIONS[lane], "fields": list(REVIEWED_CLINICAL_TRIALS_FIELDS), "page_size": config.page_size, "max_pages": config.max_pages} for lane in config.condition_lanes]
        query_set_digest = content_digest({"schema": REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, "queries": queries})
        body = {"schema": REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "query_set_digest": query_set_digest, "request_limit": config.request_limit, "record_limit": config.record_limit, "scope": "fixed_public_glioma_registry_metadata", "execution": "GET_https_clinicaltrials.gov_api_v2_studies_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(config, config.config_digest, query_set_digest, content_digest(body))

    def to_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "query_set_digest": self.query_set_digest, "request_limit": self.config.request_limit, "record_limit": self.config.record_limit, "scope": "fixed_public_glioma_registry_metadata", "execution": "GET_https_clinicaltrials.gov_api_v2_studies_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted", "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = type(self).create(self.config)
        if self != expected:
            _fail("ClinicalTrials.gov reviewed plan digest or config is invalid")


@dataclass(frozen=True, slots=True)
class ReviewedClinicalTrialsRetrievalResult:
    _bundle: dict[str, Any] = field(repr=False)
    _receipt: dict[str, Any] = field(repr=False)

    @property
    def bundle(self) -> dict[str, Any]:
        return deepcopy(self._bundle)

    @property
    def receipt(self) -> dict[str, Any]:
        return deepcopy(self._receipt)

    def to_dict(self) -> dict[str, Any]:
        return {"receipt": deepcopy(self._receipt), "retention": "metadata_only;bundle_is_transient"}

    def to_transient_dict(self) -> dict[str, Any]:
        return {"schema": REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": deepcopy(self._receipt), "retention": "caller_owned_transient_bundle"}


@dataclass(frozen=True, slots=True, init=False)
class ReviewedClinicalTrialsRetrievalAdapter:
    config: ReviewedClinicalTrialsRetrievalConfig
    _fetch: ClinicalTrialsFetcher | None

    def __init__(self, config: ReviewedClinicalTrialsRetrievalConfig, *, fetch: ClinicalTrialsFetcher | None = None):
        if type(config) is not ReviewedClinicalTrialsRetrievalConfig:
            _fail("ClinicalTrials.gov adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("ClinicalTrials.gov fetch must be callable")
        if fetch is not None and config.transport_id == BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID:
            _fail("injected ClinicalTrials.gov transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id, config.transport_version, config.transport_config_digest) != (BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID, BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION, BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST):
            _fail("built-in ClinicalTrials.gov transport identity is not exact")
        object.__setattr__(self, "config", config)
        object.__setattr__(self, "_fetch", fetch)

    def prepare(self) -> ReviewedClinicalTrialsRetrievalPlan:
        return ReviewedClinicalTrialsRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedClinicalTrialsRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedClinicalTrialsRetrievalResult:
        if type(plan) is not ReviewedClinicalTrialsRetrievalPlan:
            _fail("ClinicalTrials.gov execute requires an exact reviewed plan")
        plan.validate()
        if plan.config != self.config or plan.config_digest != self.config.config_digest:
            _fail("ClinicalTrials.gov plan differs from this adapter config")
        if approve_source_dispatch is not True:
            _fail("ClinicalTrials.gov execution requires literal source-dispatch approval")
        timestamp = _timestamp(_utc_now() if retrieved_at is None else retrieved_at, "retrieved_at")
        response_total = 0
        source_rows: list[dict[str, Any]] = []
        trial_rows: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        total_reported = 0
        omitted = 0
        request_count = 0
        for lane in self.config.condition_lanes:
            condition = REVIEWED_CLINICAL_TRIALS_CONDITIONS[lane]
            source_id = f"clinicaltrials_{lane}"
            token: str | None = None
            seen_tokens: set[str] = set()
            seen_ids: set[str] = set()
            lane_trials: list[dict[str, Any]] = []
            lane_total: int | None = None
            for page_number in range(self.config.max_pages):
                url = _url(condition, self.config.page_size, token)
                _validate_url(url, condition, self.config.page_size, token)
                try:
                    fetched = _builtin_fetch(url, float(self.config.timeout_seconds)) if self._fetch is None else self._fetch(url)
                except ReviewedClinicalTrialsRetrievalError:
                    raise
                except Exception:
                    raise ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov caller transport failed") from None
                if isinstance(fetched, bytes):
                    response_total += len(fetched)
                elif isinstance(fetched, str):
                    try:
                        response_total += len(fetched.encode("utf-8", errors="strict"))
                    except UnicodeEncodeError as error:
                        raise ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov response is not valid UTF-8 JSON") from error
                else:
                    _validate_response_tree(fetched)
                    response_total += len(_canonical_bytes(fetched, "ClinicalTrials.gov response", MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES))
                if response_total > MAX_REVIEWED_CLINICAL_TRIALS_TOTAL_RESPONSE_BYTES:
                    _fail("ClinicalTrials.gov aggregate response exceeds its byte bound")
                payload = _decode_response(fetched)
                request_count += 1
                raw_total = payload.get("totalCount")
                if raw_total is not None:
                    page_total = _integer("ClinicalTrials.gov totalCount", raw_total, 0, 9_000_000_000_000_000)
                    if lane_total is not None and page_total != lane_total:
                        _fail("ClinicalTrials.gov totalCount changed during one reviewed page sequence")
                    lane_total = page_total
                studies = payload.get("studies")
                if not isinstance(studies, list) or len(studies) > self.config.page_size:
                    _fail("ClinicalTrials.gov page has an invalid studies list")
                for study in studies:
                    row = _trial(study, source_id)
                    if row["nct_id"] in seen_ids:
                        _fail("ClinicalTrials.gov returned a duplicate NCT identifier")
                    seen_ids.add(row["nct_id"])
                    lane_trials.append(row)
                raw_token = payload.get("nextPageToken")
                if raw_token is None or raw_token == "":
                    token = None
                    break
                if not isinstance(raw_token, str) or not _TOKEN.fullmatch(raw_token) or raw_token in seen_tokens:
                    _fail("ClinicalTrials.gov pagination token is malformed or cyclic")
                seen_tokens.add(raw_token)
                token = raw_token
                if page_number + 1 == self.config.max_pages:
                    break
            lane_trials.sort(key=lambda item: item["nct_id"])
            if lane_total is not None and lane_total < len(lane_trials):
                _fail("ClinicalTrials.gov totalCount is smaller than the returned study count")
            truncated = token is not None or (lane_total is not None and len(lane_trials) < lane_total)
            if lane_total is not None:
                total_reported += lane_total
            if lane_total is not None and lane_total > len(lane_trials):
                omitted += lane_total - len(lane_trials)
            if total_reported > 9_000_000_000_000_000 or omitted > 9_000_000_000_000_000:
                _fail("ClinicalTrials.gov aggregate totals exceed the portable integer contract")
            source_digest = content_digest(lane_trials)
            source_rows.append({"source_id": source_id, "authority": REVIEWED_CLINICAL_TRIALS_AUTHORITY, "uri": _SOURCE_BASE, "retrieved_at": timestamp, "content_sha256": source_digest, "record_count": len(lane_trials)})
            source_receipts.append({"schema": REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA, "lane": lane, "source_id": source_id, "content_digest": source_digest, "record_count": len(lane_trials), "reported_total_count": lane_total, "truncated": truncated})
            trial_rows.extend(lane_trials)
        trial_rows.sort(key=lambda item: (item["source_id"], item["nct_id"]))
        unsigned_bundle = {"schema": REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": source_rows, "trials": trial_rows, "source_set_digest": content_digest(source_rows), "record_count": len(trial_rows), "reported_total_count": total_reported if source_rows and all(row["reported_total_count"] is not None for row in source_receipts) else None, "omitted_record_count": omitted, "truncated": any(row["truncated"] for row in source_receipts), "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _canonical_bytes(unsigned_bundle, "ClinicalTrials.gov transient bundle", MAX_REVIEWED_CLINICAL_TRIALS_BUNDLE_BYTES)
        bundle = {**unsigned_bundle, "bundle_digest": content_digest(unsigned_bundle)}
        receipt_payload = {"schema": REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "query_set_digest": plan.query_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": unsigned_bundle["source_set_digest"], "source_count": len(source_rows), "record_count": len(trial_rows), "request_count": request_count, "reported_total_count": unsigned_bundle["reported_total_count"], "omitted_record_count": omitted, "truncated": unsigned_bundle["truncated"], "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_payload, "receipt_digest": content_digest(receipt_payload)}
        return ReviewedClinicalTrialsRetrievalResult(bundle, receipt)


def create_reviewed_clinical_trials_execution_metadata(plan: ReviewedClinicalTrialsRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedClinicalTrialsRetrievalPlan:
        _fail("ClinicalTrials.gov execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("ClinicalTrials.gov execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "retrieved_at")
    payload = {"schema": REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _exact_mapping(value: Any, name: str, keys: set[str]) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != keys:
        _fail(f"{name} has an invalid shape")
    return value


def _validate_transient(value: Any, plan: ReviewedClinicalTrialsRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    transient = _exact_mapping(value, "generic ClinicalTrials.gov transient value", {"schema", "bundle", "receipt", "retention"})
    if transient["schema"] != REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA or transient["retention"] != "caller_owned_transient_bundle":
        _fail("generic ClinicalTrials.gov transient value identity is invalid")
    bundle_keys = {"schema", "generated_at", "sources", "trials", "source_set_digest", "record_count", "reported_total_count", "omitted_record_count", "truncated", "provider", "credentials", "limitations", "bundle_digest"}
    bundle = _exact_mapping(transient["bundle"], "generic ClinicalTrials.gov bundle", bundle_keys)
    receipt_keys = {"schema", "plan_digest", "config_digest", "query_set_digest", "bundle_digest", "source_set_digest", "source_count", "record_count", "request_count", "reported_total_count", "omitted_record_count", "truncated", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    receipt = _exact_mapping(transient["receipt"], "generic ClinicalTrials.gov receipt", receipt_keys)

    bundle_digest = _digest("bundle_digest", bundle["bundle_digest"])
    receipt_digest = _digest("receipt_digest", receipt["receipt_digest"])
    if _bundle_digest(bundle) != bundle_digest:
        _fail("generic ClinicalTrials.gov bundle digest is invalid")
    unsigned_receipt = dict(receipt)
    unsigned_receipt.pop("receipt_digest")
    if content_digest(unsigned_receipt) != receipt_digest:
        _fail("generic ClinicalTrials.gov receipt digest is invalid")

    if bundle["schema"] != REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA or receipt["schema"] != REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA:
        _fail("generic ClinicalTrials.gov bundle or receipt schema is invalid")
    if (receipt["plan_digest"], receipt["config_digest"], receipt["query_set_digest"]) != (plan.plan_digest, plan.config_digest, plan.query_set_digest):
        _fail("generic ClinicalTrials.gov receipt is not bound to its reviewed plan")
    if receipt["bundle_digest"] != bundle_digest or receipt["source_set_digest"] != bundle["source_set_digest"]:
        _fail("generic ClinicalTrials.gov receipt does not bind its bundle")
    if bundle["provider"] != "none" or receipt["provider"] != "none" or bundle["credentials"] != "not_accepted" or receipt["credentials"] != "not_accepted" or receipt["effect"] != "read_only" or receipt["retention"] != _RETENTION or receipt["network"] != expected_network:
        _fail("generic ClinicalTrials.gov bundle or receipt exceeds the reviewed source boundary")
    if bundle["limitations"] != list(_LIMITATIONS) or receipt["limitations"] != list(_LIMITATIONS):
        _fail("generic ClinicalTrials.gov limitations differ from the reviewed contract")

    lanes = plan.config.condition_lanes
    sources = bundle["sources"]
    trials = bundle["trials"]
    source_receipts = receipt["source_receipts"]
    if not isinstance(sources, list) or len(sources) != len(lanes) or not isinstance(source_receipts, list) or len(source_receipts) != len(lanes):
        _fail("generic ClinicalTrials.gov source inventory is incomplete")
    if not isinstance(trials, list) or len(trials) > plan.config.record_limit:
        _fail("generic ClinicalTrials.gov trial inventory exceeds its reviewed bound")
    expected_source_ids = [f"clinicaltrials_{lane}" for lane in lanes]
    if any(not isinstance(row, Mapping) or row.get("source_id") not in expected_source_ids for row in trials):
        _fail("generic ClinicalTrials.gov trial inventory contains an unknown source")
    _timestamp(bundle["generated_at"], "bundle.generated_at")
    if receipt["retrieved_at"] != bundle["generated_at"]:
        _fail("generic ClinicalTrials.gov timestamps do not match")

    trial_keys = {"source_id", "nct_id", "title", "overall_status", "phases", "last_update", "study_type", "enrollment_count", "intervention_names"}
    source_keys = {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count"}
    source_receipt_keys = {"schema", "lane", "source_id", "content_digest", "record_count", "reported_total_count", "truncated"}
    if [row.get("source_id") if isinstance(row, Mapping) else None for row in sources] != expected_source_ids:
        _fail("generic ClinicalTrials.gov source order or identity is invalid")

    omitted = 0
    reported = 0
    all_reported = True
    any_truncated = False
    for index, lane in enumerate(lanes):
        source_id = expected_source_ids[index]
        source = _exact_mapping(sources[index], "generic ClinicalTrials.gov source", source_keys)
        source_receipt = _exact_mapping(source_receipts[index], "generic ClinicalTrials.gov source receipt", source_receipt_keys)
        if source_receipt["schema"] != REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA or source_receipt["lane"] != lane or source_receipt["source_id"] != source_id:
            _fail("generic ClinicalTrials.gov source receipt identity is invalid")
        if source["authority"] != REVIEWED_CLINICAL_TRIALS_AUTHORITY or source["uri"] != _SOURCE_BASE or source["source_id"] != source_id or source["retrieved_at"] != bundle["generated_at"]:
            _fail("generic ClinicalTrials.gov source metadata is invalid")
        content_sha = _digest("source.content_sha256", source["content_sha256"])
        if _digest("source_receipt.content_digest", source_receipt["content_digest"]) != content_sha:
            _fail("generic ClinicalTrials.gov source receipt digest does not match")
        lane_rows = [row for row in trials if isinstance(row, Mapping) and row.get("source_id") == source_id]
        if lane_rows != sorted(lane_rows, key=lambda row: row.get("nct_id", "")):
            _fail("generic ClinicalTrials.gov trial ordering is invalid")
        nct_ids: set[str] = set()
        for row in lane_rows:
            trial = _exact_mapping(row, "generic ClinicalTrials.gov trial", trial_keys)
            nct_id = _text("trial.nct_id", trial["nct_id"], 16)
            if nct_id is None or not _NCT.fullmatch(nct_id) or nct_id in nct_ids:
                _fail("generic ClinicalTrials.gov trial identifier is invalid or duplicated")
            nct_ids.add(nct_id)
            if _text("trial.source_id", trial["source_id"], 128) != trial["source_id"]:
                _fail("generic ClinicalTrials.gov trial source id is not normalized")
            if _text("trial.title", trial["title"], 4096) != trial["title"] or _text("trial.overall_status", trial["overall_status"], 128) != trial["overall_status"]:
                _fail("generic ClinicalTrials.gov trial text is not normalized")
            phases = trial["phases"]
            if _list_text(phases, "trial.phases", 16) != phases:
                _fail("generic ClinicalTrials.gov trial phases are not normalized")
            if trial["last_update"] is not None:
                _trial_date = _text("trial.last_update", trial["last_update"], 10)
                if _trial_date != trial["last_update"] or not _DATE.fullmatch(_trial_date or ""):
                    _fail("generic ClinicalTrials.gov trial date is invalid")
                try:
                    datetime.strptime(_trial_date, "%Y-%m") if len(_trial_date) == 7 else date.fromisoformat(_trial_date)
                except ValueError as error:
                    raise ReviewedClinicalTrialsRetrievalError("generic ClinicalTrials.gov trial date is invalid") from error
            if trial["study_type"] is not None:
                if _text("trial.study_type", trial["study_type"], 128) != trial["study_type"]:
                    _fail("generic ClinicalTrials.gov trial study type is not normalized")
            if trial["enrollment_count"] is not None:
                _integer("trial.enrollment_count", trial["enrollment_count"], 0, 10_000_000)
            names = trial["intervention_names"]
            if _list_text(names, "trial.intervention_names", 128) != names:
                _fail("generic ClinicalTrials.gov trial interventions are not normalized")
        source_record_count = _integer("source.record_count", source["record_count"], 0, plan.config.record_limit)
        source_receipt_record_count = _integer("source_receipt.record_count", source_receipt["record_count"], 0, plan.config.record_limit)
        if source_record_count != len(lane_rows) or source_receipt_record_count != len(lane_rows):
            _fail("generic ClinicalTrials.gov source record count is invalid")
        if content_digest(lane_rows) != content_sha:
            _fail("generic ClinicalTrials.gov source content digest is invalid")
        total = source_receipt["reported_total_count"]
        if total is None:
            all_reported = False
        else:
            total = _integer("source_receipt.reported_total_count", total, 0, 9_000_000_000_000_000)
            if total < len(lane_rows):
                _fail("generic ClinicalTrials.gov reported total is smaller than returned records")
            reported += total
            omitted += total - len(lane_rows)
            if reported > 9_000_000_000_000_000 or omitted > 9_000_000_000_000_000:
                _fail("generic ClinicalTrials.gov aggregate totals exceed the portable integer contract")
        if not isinstance(source_receipt["truncated"], bool):
            _fail("generic ClinicalTrials.gov truncation state is invalid")
        if total is not None and total > len(lane_rows) and not source_receipt["truncated"]:
            _fail("generic ClinicalTrials.gov truncation state hides omitted source records")
        any_truncated = any_truncated or source_receipt["truncated"]

    request_count = _integer("receipt.request_count", receipt["request_count"], len(lanes), plan.config.request_limit)
    del request_count
    source_count = _integer("receipt.source_count", receipt["source_count"], 0, len(lanes))
    receipt_record_count = _integer("receipt.record_count", receipt["record_count"], 0, plan.config.record_limit)
    bundle_record_count = _integer("bundle.record_count", bundle["record_count"], 0, plan.config.record_limit)
    if source_count != len(lanes) or receipt_record_count != len(trials) or bundle_record_count != len(trials):
        _fail("generic ClinicalTrials.gov aggregate counts are invalid")
    if trials != sorted(trials, key=lambda row: (row["source_id"], row["nct_id"])):
        _fail("generic ClinicalTrials.gov aggregate trial ordering is invalid")
    if bundle["source_set_digest"] != content_digest(sources):
        _fail("generic ClinicalTrials.gov source-set digest is invalid")
    expected_reported = reported if all_reported else None
    if bundle["reported_total_count"] != expected_reported or receipt["reported_total_count"] != expected_reported:
        _fail("generic ClinicalTrials.gov reported total is inconsistent")
    bundle_omitted = _integer("bundle.omitted_record_count", bundle["omitted_record_count"], 0, 9_000_000_000_000_000)
    receipt_omitted = _integer("receipt.omitted_record_count", receipt["omitted_record_count"], 0, 9_000_000_000_000_000)
    if bundle_omitted != omitted or receipt_omitted != omitted or bundle["truncated"] is not any_truncated or receipt["truncated"] is not any_truncated:
        _fail("generic ClinicalTrials.gov truncation accounting is inconsistent")
    return bundle, receipt


def create_reviewed_clinical_trials_autonomous_evidence_registration(adapter: ReviewedClinicalTrialsRetrievalAdapter, plan: ReviewedClinicalTrialsRetrievalPlan, *, condition_lane: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedClinicalTrialsRetrievalAdapter or type(plan) is not ReviewedClinicalTrialsRetrievalPlan:
        _fail("ClinicalTrials.gov registration requires exact adapter and plan values")
    plan.validate()
    if adapter.config != plan.config:
        _fail("ClinicalTrials.gov registration adapter and plan configs differ")
    if condition_lane not in plan.config.condition_lanes or plan.config.condition_lanes != (condition_lane,):
        _fail("generic ClinicalTrials.gov registration requires the exact single reviewed lane")
    source_id = f"clinicaltrials_{condition_lane}"
    frozen_plan = ReviewedClinicalTrialsRetrievalPlan.create(ReviewedClinicalTrialsRetrievalConfig(**{
        "condition_lanes": plan.config.condition_lanes,
        "page_size": plan.config.page_size,
        "max_pages": plan.config.max_pages,
        "timeout_seconds": plan.config.timeout_seconds,
        "transport_id": plan.config.transport_id,
        "transport_version": plan.config.transport_version,
        "transport_config_digest": plan.config.transport_config_digest,
    }))
    expected_network = "builtin_https" if adapter._fetch is None else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("generic ClinicalTrials.gov request does not match its reviewed source")
        metadata = request.get("metadata")
        if not isinstance(metadata, Mapping) or set(metadata) != {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}:
            _fail("generic ClinicalTrials.gov execution metadata is malformed")
        unsigned = dict(metadata)
        supplied = _digest("metadata_digest", unsigned.pop("metadata_digest"))
        if supplied != content_digest(unsigned) or metadata.get("schema") != REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("generic ClinicalTrials.gov execution metadata failed review binding")
        timestamp = None if metadata["retrieved_at"] is None else _timestamp(metadata["retrieved_at"], "retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=timestamp).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        label = _text("ClinicalTrials.gov requirement label", _path(context, "requirement", "label"), 512)
        assert label is not None
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(adapter_id=f"clinicaltrials_{condition_lane}_{plan.plan_digest[:16]}", version=REVIEWED_CLINICAL_TRIALS_ADAPTER_VERSION, domains=("biomedical", "neuroscience"), capabilities=("evidence", "provenance", "clinical_trials_registry"), source_kinds=("clinicaltrials", "public_registry"), acquire=acquire, project=project)


__all__ = [
    "REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA", "REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA",
    "REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA", "REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA",
    "REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA", "REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA",
    "REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA", "REVIEWED_CLINICAL_TRIALS_ADAPTER_VERSION",
    "REVIEWED_CLINICAL_TRIALS_HOST", "REVIEWED_CLINICAL_TRIALS_PATH", "REVIEWED_CLINICAL_TRIALS_AUTHORITY", "REVIEWED_CLINICAL_TRIALS_FIELDS",
    "REVIEWED_CLINICAL_TRIALS_CONDITIONS", "MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE",
    "MAX_REVIEWED_CLINICAL_TRIALS_PAGES", "MAX_REVIEWED_CLINICAL_TRIALS_RECORDS",
    "MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES", "MAX_REVIEWED_CLINICAL_TRIALS_TOTAL_RESPONSE_BYTES",
    "MAX_REVIEWED_CLINICAL_TRIALS_BUNDLE_BYTES", "MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH",
    "MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES", "BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID",
    "BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION", "BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST",
    "ReviewedClinicalTrialsRetrievalError", "ClinicalTrialsFetcher", "ReviewedClinicalTrialsRetrievalConfig",
    "ReviewedClinicalTrialsRetrievalPlan", "ReviewedClinicalTrialsRetrievalResult",
    "ReviewedClinicalTrialsRetrievalAdapter", "create_reviewed_clinical_trials_execution_metadata",
    "create_reviewed_clinical_trials_autonomous_evidence_registration",
]
