"""Reviewed, bounded retrieval of fixed public cBioPortal study/profile metadata.

The adapter reads fixed public study summaries and molecular-profile catalogues after literal
caller approval. It never requests samples, patients, clinical rows, or molecular values. Raw
catalogue metadata remains transient; autonomous evidence receives only digests and limitations.

Blueprint 23 (Adult Diffuse Glioma and Glioblastoma Worlds) calls for redistributable public
metadata or a bring-your-own-data connector. This module supplies only a bounded cBioPortal
catalogue adapter; it does not implement the temporal parent world, specimen lineage, or its
evaluation and release gates.
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


REVIEWED_CBIOPORTAL_CONFIG_SCHEMA = "bioprism-reviewed-cbioportal-study-config/0.1"
REVIEWED_CBIOPORTAL_PLAN_SCHEMA = "bioprism-reviewed-cbioportal-study-plan/0.1"
REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-cbioportal-study-source-receipt/0.1"
REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA = "bioprism-reviewed-cbioportal-study-bundle/0.1"
REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA = "bioprism-reviewed-cbioportal-study-receipt/0.1"
REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA = "bioprism-reviewed-cbioportal-study-transient/0.1"
REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-cbioportal-study-execution-metadata/0.1"
REVIEWED_CBIOPORTAL_ADAPTER_VERSION = "0.1"
REVIEWED_CBIOPORTAL_HOST = "www.cbioportal.org"
REVIEWED_CBIOPORTAL_PATH = "/api/studies"
REVIEWED_CBIOPORTAL_ENDPOINT = f"https://{REVIEWED_CBIOPORTAL_HOST}{REVIEWED_CBIOPORTAL_PATH}"
REVIEWED_CBIOPORTAL_AUTHORITY = "cBioPortal for Cancer Genomics"
REVIEWED_CBIOPORTAL_STUDIES = MappingProxyType({"gbm": "gbm_tcga", "lgg": "lgg_tcga"})
MAX_REVIEWED_CBIOPORTAL_STUDIES = 2
MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY = 128
MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES = 1_000_000
MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES = 4_000_000
MAX_REVIEWED_CBIOPORTAL_BUNDLE_BYTES = 2_000_000
MAX_REVIEWED_CBIOPORTAL_TREE_DEPTH = 32
MAX_REVIEWED_CBIOPORTAL_TREE_NODES = 50_000
BUILTIN_CBIOPORTAL_TRANSPORT_ID = "builtin.nci-cbioportal.urllib"
BUILTIN_CBIOPORTAL_TRANSPORT_VERSION = "1"
BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST = content_digest({
    "implementation": "urllib.request",
    "scheme": "https",
    "host": REVIEWED_CBIOPORTAL_HOST,
    "paths": ["/api/studies/{studyId}", "/api/studies/{studyId}/molecular-profiles"],
    "method": "GET",
    "study_fields": ["studyId", "cancerTypeId", "name", "description", "publicStudy", "pmid", "allSampleCount", "referenceGenome", "importDate"],
    "profile_projection": "SUMMARY",
    "profile_page_size": MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY,
    "profile_page_number": 0,
    "profile_sort": ["molecularProfileId", "ASC"],
    "redirects": "refused",
    "credentials": "not_accepted",
})

_RETENTION = "public_study_and_molecular_profile_metadata_only;no_sample_patient_or_molecular_rows"
_LIMITATIONS = (
    "cBioPortal study summaries and molecular profile definitions are catalogue metadata, not patient-level evidence or outcome",
    "aggregate sample counts and molecular profile definitions are source-reported and may change between retrievals",
    "the fixed TCGA GBM and LGG catalogue is not an exhaustive glioma cohort search",
    "independent review is required for freshness, omissions, study quality, and applicability",
    "the adapter does not request or retain sample, patient, clinical, genomic, assay-value, or controlled-access records",
    "the public cBioPortal API is documented as beta and may change; endpoint and response drift fail closed",
    "caller-injected transports own timeout, redirect, and network policy under their declared identity",
)
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
_PROFILE_ID_RE = re.compile(r"^[A-Za-z0-9_.:-]{1,256}$")
_STUDY_TO_LANE = {study_id: lane for lane, study_id in REVIEWED_CBIOPORTAL_STUDIES.items()}
_PROFILE_ALTERATIONS = frozenset({"MUTATION_EXTENDED", "MUTATION_UNCALLED", "STRUCTURAL_VARIANT", "COPY_NUMBER_ALTERATION", "MICRO_RNA_EXPRESSION", "MRNA_EXPRESSION", "MRNA_EXPRESSION_NORMALS", "RNA_EXPRESSION", "METHYLATION", "METHYLATION_BINARY", "PHOSPHORYLATION", "PROTEIN_LEVEL", "PROTEIN_ARRAY_PROTEIN_LEVEL", "PROTEIN_ARRAY_PHOSPHORYLATION", "GENESET_SCORE", "GENERIC_ASSAY"})


class ReviewedCBioPortalRetrievalError(ArgumentError):
    """A CBIOPORTAL response or reviewed study plan failed its bounded contract."""


CbioPortalFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


def _fail(message: str) -> None:
    raise ReviewedCBioPortalRetrievalError(message)


def _json_bytes(value: Any, name: str, maximum: int) -> bytes:
    try:
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ReviewedCBioPortalRetrievalError(f"{name} is not bounded JSON") from error
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
        raise ReviewedCBioPortalRetrievalError(f"{name} contains invalid Unicode") from error
    if (not normalized and not optional) or len(encoded) > maximum or any(ord(char) < 32 or ord(char) == 127 for char in normalized):
        _fail(f"{name} is empty or exceeds its text bound")
    return normalized or None


def _timestamp(value: Any, name: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        _fail(f"{name} must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ReviewedCBioPortalRetrievalError(f"{name} is not a real UTC timestamp") from error
    return value


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).strftime("%Y-%m-%dT%H:%M:%SZ")


def _validate_tree(value: Any, *, depth: int = 0, budget: list[int] | None = None) -> None:
    if budget is None:
        budget = [MAX_REVIEWED_CBIOPORTAL_TREE_NODES]
    budget[0] -= 1
    if budget[0] < 0 or depth > MAX_REVIEWED_CBIOPORTAL_TREE_DEPTH:
        _fail("CBIOPORTAL response exceeds its structural bound")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("CBIOPORTAL response contains a non-text key")
            try:
                key.encode("utf-8")
            except UnicodeEncodeError as error:
                raise ReviewedCBioPortalRetrievalError("CBIOPORTAL response contains invalid Unicode") from error
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, list):
        for child in value:
            _validate_tree(child, depth=depth + 1, budget=budget)
    elif isinstance(value, str):
        try:
            value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedCBioPortalRetrievalError("CBIOPORTAL response contains invalid Unicode") from error
    elif isinstance(value, float) and not math.isfinite(value):
        _fail("CBIOPORTAL response contains a non-finite number")
    elif value is not None and not isinstance(value, (str, int, float, bool)):
        _fail("CBIOPORTAL response contains an unsupported value")


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    output: dict[str, Any] = {}
    for key, value in pairs:
        if key in output:
            _fail("CBIOPORTAL response contains duplicate JSON fields")
        output[key] = value
    return output


def _parse_response(value: Any) -> tuple[Any, int]:
    if isinstance(value, (Mapping, list)):
        encoded = _json_bytes(value, "CBIOPORTAL response", MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES)
        parsed: Any = value
    elif isinstance(value, str):
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as error:
            raise ReviewedCBioPortalRetrievalError("CBIOPORTAL response is not valid UTF-8") from error
        if len(encoded) > MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES:
            _fail("CBIOPORTAL response exceeds its byte bound")
        try:
            parsed = json.loads(encoded, object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("CBIOPORTAL response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedCBioPortalRetrievalError("CBIOPORTAL response is not valid JSON") from error
    elif isinstance(value, bytes):
        encoded = value
        if len(encoded) > MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES:
            _fail("CBIOPORTAL response exceeds its byte bound")
        try:
            parsed = json.loads(encoded.decode("utf-8"), object_pairs_hook=_pairs_no_duplicates, parse_constant=lambda _v: _fail("CBIOPORTAL response contains a non-finite number"))
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            raise ReviewedCBioPortalRetrievalError("CBIOPORTAL response is not valid JSON") from error
    else:
        _fail("CBIOPORTAL response is not JSON")
    if not isinstance(parsed, (Mapping, list)):
        _fail("cBioPortal response root is not an object or array")
    _validate_tree(parsed)
    return parsed, len(encoded)


def _study_url(study_id: str) -> str:
    return f"{REVIEWED_CBIOPORTAL_ENDPOINT}/{study_id}"


def _profiles_url(study_id: str) -> str:
    query = urlencode({
        "projection": "SUMMARY",
        "pageSize": MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY,
        "pageNumber": 0,
        "sortBy": "molecularProfileId",
        "direction": "ASC",
    })
    return f"{REVIEWED_CBIOPORTAL_ENDPOINT}/{study_id}/molecular-profiles?{query}"


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: Any, **_kwargs: Any) -> None:
        return None


def _builtin_fetch(url: str, timeout_ms: int) -> bytes:
    if not (url.startswith(f"{REVIEWED_CBIOPORTAL_ENDPOINT}/") and ("/molecular-profiles?" in url or "?" not in url)):
        _fail("CBIOPORTAL built-in transport refused an unpinned URL")
    request = Request(url, headers={"Accept": "application/json"}, method="GET")
    opener = build_opener(_NoRedirect)
    try:
        with opener.open(request, timeout=timeout_ms / 1000) as response:
            if response.geturl() != url or response.status != 200:
                _fail("CBIOPORTAL endpoint returned an unexpected response")
            body = response.read(MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError) as error:
        raise ReviewedCBioPortalRetrievalError("CBIOPORTAL study request failed") from error
    if len(body) > MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES:
        _fail("CBIOPORTAL response exceeds its byte bound")
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


def _study(raw: Any, study_id: str) -> dict[str, Any]:
    if not isinstance(raw, Mapping):
        _fail("cBioPortal study response is malformed")
    if raw.get("studyId") != study_id or raw.get("publicStudy") is not True:
        _fail("cBioPortal study identity or public status does not match the reviewed study")
    lane = _STUDY_TO_LANE[study_id]
    sample_count = _count("cBioPortal allSampleCount", raw.get("allSampleCount"), optional=True)
    return {
        "source_id": f"cbioportal_{lane}",
        "lane": lane,
        "study_id": study_id,
        "name": _text("cBioPortal study name", raw.get("name"), 2_000),
        "description": _text("cBioPortal study description", raw.get("description"), 8_000, optional=True),
        "cancer_type_id": _text("cBioPortal cancerTypeId", raw.get("cancerTypeId"), 128, optional=True),
        "public_study": True,
        "pmid": _text("cBioPortal study PMID", raw.get("pmid"), 128, optional=True),
        "sample_count": sample_count,
        "reference_genome": _text("cBioPortal referenceGenome", raw.get("referenceGenome"), 128, optional=True),
        "import_date": _text("cBioPortal importDate", raw.get("importDate"), 64, optional=True),
        "metadata_completeness": "complete" if sample_count is not None else "unknown",
    }


def _profiles(raw: Any, study_id: str) -> list[dict[str, Any]]:
    if not isinstance(raw, list) or len(raw) > MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY:
        _fail("cBioPortal molecular-profile response exceeds its catalogue bound")
    result: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, Mapping) or item.get("studyId") != study_id:
            _fail("cBioPortal molecular profile has a mismatched study identity")
        profile_id = _text("cBioPortal molecularProfileId", item.get("molecularProfileId"), 256)
        if profile_id is None or not _PROFILE_ID_RE.fullmatch(profile_id):
            _fail("cBioPortal molecularProfileId is outside its fixed identifier bound")
        if profile_id in seen:
            _fail("cBioPortal molecular profile catalogue contains duplicate IDs")
        seen.add(profile_id)
        alteration = _text("cBioPortal molecularAlterationType", item.get("molecularAlterationType"), 64)
        if alteration not in _PROFILE_ALTERATIONS:
            _fail("cBioPortal molecular profile has an unsupported alteration type")
        for field_name in ("showProfileInAnalysisTab", "patientLevel"):
            if item.get(field_name) is not None and not isinstance(item[field_name], bool):
                _fail(f"cBioPortal {field_name} must be a boolean or unknown")
        result.append({
            "study_id": study_id,
            "profile_id": profile_id,
            "name": _text("cBioPortal profile name", item.get("name"), 2_000),
            "molecular_alteration_type": alteration,
            "generic_assay_type": _text("cBioPortal genericAssayType", item.get("genericAssayType"), 128, optional=True),
            "datatype": _text("cBioPortal profile datatype", item.get("datatype"), 128),
            "description": _text("cBioPortal profile description", item.get("description"), 4_000, optional=True),
            "show_in_analysis": item.get("showProfileInAnalysisTab"),
            "patient_level": item.get("patientLevel"),
        })
    return sorted(result, key=lambda profile: profile["profile_id"])


@dataclass(frozen=True, slots=True)
class ReviewedCBioPortalRetrievalConfig:
    study_ids: tuple[str, ...] = ("gbm_tcga",)
    timeout_ms: int = 30_000
    transport_id: str = BUILTIN_CBIOPORTAL_TRANSPORT_ID
    transport_version: str = BUILTIN_CBIOPORTAL_TRANSPORT_VERSION
    transport_config_digest: str = BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST
    _study_set_digest: str = field(default="", repr=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.study_ids, Sequence) or isinstance(self.study_ids, (str, bytes)):
            _fail("CBIOPORTAL study_ids must be a bounded list")
        studies = tuple(self.study_ids)
        if not studies or len(studies) > MAX_REVIEWED_CBIOPORTAL_STUDIES or any(not isinstance(study, str) or study not in _STUDY_TO_LANE for study in studies) or len(set(studies)) != len(studies):
            _fail("CBIOPORTAL study_ids contain an unsupported or duplicate study")
        canonical_studies = tuple(study for study in REVIEWED_CBIOPORTAL_STUDIES.values() if study in studies)
        object.__setattr__(self, "study_ids", canonical_studies)
        object.__setattr__(self, "timeout_ms", _integer("CBIOPORTAL timeout_ms", self.timeout_ms, 100, 120_000))
        for name in ("transport_id", "transport_version"):
            value = getattr(self, name)
            if not isinstance(value, str) or not _IDENTIFIER_RE.fullmatch(value):
                _fail(f"CBIOPORTAL {name} is invalid")
        _digest("CBIOPORTAL transport_config_digest", self.transport_config_digest)
        study_digest = content_digest(self._study_payload(canonical_studies))
        if self._study_set_digest and _digest("cBioPortal study_set_digest", self._study_set_digest) != study_digest:
            _fail("CBIOPORTAL study_set_digest does not match the fixed study catalogue")
        object.__setattr__(self, "_study_set_digest", study_digest)
        _json_bytes(self.to_dict(), "CBIOPORTAL config", 32_000)

    @staticmethod
    def _study_payload(studies: Sequence[str]) -> dict[str, Any]:
        return {"studies": [{"lane": _STUDY_TO_LANE[study], "study_id": study} for study in studies]}

    @property
    def request_limit(self) -> int:
        return len(self.study_ids) * 2

    @property
    def config_digest(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": REVIEWED_CBIOPORTAL_CONFIG_SCHEMA,
            "study_ids": list(self.study_ids),
            "timeout_ms": self.timeout_ms,
            "request_limit": self.request_limit,
            "transport_id": self.transport_id,
            "transport_version": self.transport_version,
            "transport_config_digest": self.transport_config_digest,
            "study_set_digest": self._study_set_digest,
            "retention": _RETENTION,
            "credentials": "not_accepted",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "config_digest": self.config_digest}

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedCBioPortalRetrievalConfig":
        expected = {"schema", "study_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "study_set_digest", "retention", "credentials", "config_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_CBIOPORTAL_CONFIG_SCHEMA:
            _fail("CBIOPORTAL config has an invalid shape")
        config = cls(tuple(raw["study_ids"]), raw["timeout_ms"], raw["transport_id"], raw["transport_version"], raw["transport_config_digest"], raw["study_set_digest"])
        if canonical_json(config.to_dict()) != canonical_json(dict(raw)):
            _fail("CBIOPORTAL config is not normalized or its digest is invalid")
        return config


@dataclass(frozen=True, slots=True)
class ReviewedCBioPortalRetrievalPlan:
    config: ReviewedCBioPortalRetrievalConfig
    config_digest: str
    study_set_digest: str
    plan_digest: str

    @classmethod
    def create(cls, config: ReviewedCBioPortalRetrievalConfig) -> "ReviewedCBioPortalRetrievalPlan":
        if type(config) is not ReviewedCBioPortalRetrievalConfig:
            _fail("CBIOPORTAL plan requires an exact config")
        unsigned = {"schema": REVIEWED_CBIOPORTAL_PLAN_SCHEMA, "config": config.to_dict(), "config_digest": config.config_digest, "study_set_digest": config._study_set_digest, "request_limit": config.request_limit, "scope": "fixed_public_cbioportal_study_and_profile_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return cls(config, config.config_digest, config._study_set_digest, content_digest(unsigned))

    def to_dict(self) -> dict[str, Any]:
        unsigned = {"schema": REVIEWED_CBIOPORTAL_PLAN_SCHEMA, "config": self.config.to_dict(), "config_digest": self.config_digest, "study_set_digest": self.study_set_digest, "request_limit": self.config.request_limit, "scope": "fixed_public_cbioportal_study_and_profile_metadata", "execution": "bounded_https_get_after_literal_approval", "retention": _RETENTION, "credentials": "not_accepted"}
        return {**unsigned, "plan_digest": self.plan_digest}

    def validate(self) -> None:
        expected = self.create(self.config)
        if (self.config_digest, self.study_set_digest, self.plan_digest) != (expected.config_digest, expected.study_set_digest, expected.plan_digest):
            _fail("CBIOPORTAL plan has drifted from its reviewed identity")

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "ReviewedCBioPortalRetrievalPlan":
        expected = {"schema", "config", "config_digest", "study_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"}
        if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") != REVIEWED_CBIOPORTAL_PLAN_SCHEMA:
            _fail("CBIOPORTAL plan has an invalid shape")
        plan = cls.create(ReviewedCBioPortalRetrievalConfig.from_dict(raw["config"]))
        if canonical_json(plan.to_dict()) != canonical_json(dict(raw)):
            _fail("CBIOPORTAL plan is not normalized or its digest is invalid")
        return plan


@dataclass(frozen=True, slots=True)
class ReviewedCBioPortalRetrievalResult:
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
        return {"schema": REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA, "bundle": self.bundle, "receipt": self.receipt, "retention": "caller_owned_transient_study_profile_metadata"}


class ReviewedCBioPortalRetrievalAdapter:
    """Read only fixed cBioPortal study and molecular-profile metadata after review."""

    def __init__(self, config: ReviewedCBioPortalRetrievalConfig, *, fetch: CbioPortalFetcher | None = None) -> None:
        if type(config) is not ReviewedCBioPortalRetrievalConfig:
            _fail("CBIOPORTAL adapter requires an exact config")
        if fetch is not None and not callable(fetch):
            _fail("CBIOPORTAL injected transport is malformed")
        if fetch is not None and config.transport_id == BUILTIN_CBIOPORTAL_TRANSPORT_ID:
            _fail("CBIOPORTAL injected transport requires a distinct reviewed identity")
        if fetch is None and (config.transport_id != BUILTIN_CBIOPORTAL_TRANSPORT_ID or config.transport_config_digest != BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST):
            _fail("CBIOPORTAL built-in transport identity is not exact")
        self.config = config
        self._fetch = fetch

    def prepare(self) -> ReviewedCBioPortalRetrievalPlan:
        return ReviewedCBioPortalRetrievalPlan.create(self.config)

    def execute(self, plan: ReviewedCBioPortalRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> ReviewedCBioPortalRetrievalResult:
        if type(plan) is not ReviewedCBioPortalRetrievalPlan:
            _fail("CBIOPORTAL execution requires an exact reviewed plan")
        plan.validate()
        if canonical_json(plan.config.to_dict()) != canonical_json(self.config.to_dict()):
            _fail("CBIOPORTAL execution config differs from its reviewed plan")
        if approve_source_dispatch is not True:
            _fail("CBIOPORTAL dispatch requires literal approval")
        timestamp = _now() if retrieved_at is None else _timestamp(retrieved_at, "CBIOPORTAL retrieved_at")
        studies: list[dict[str, Any]] = []
        molecular_profiles: list[dict[str, Any]] = []
        sources: list[dict[str, Any]] = []
        source_receipts: list[dict[str, Any]] = []
        response_bytes = 0
        for study_id in self.config.study_ids:
            try:
                study_raw = _builtin_fetch(_study_url(study_id), self.config.timeout_ms) if self._fetch is None else self._fetch(_study_url(study_id))
            except ReviewedCBioPortalRetrievalError:
                raise
            except Exception as error:
                raise ReviewedCBioPortalRetrievalError("cBioPortal metadata request failed") from error
            study_response, study_bytes = _parse_response(study_raw)
            response_bytes += study_bytes
            if response_bytes > MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES:
                _fail("cBioPortal aggregate response bytes exceed the plan bound")
            study = _study(study_response, study_id)
            try:
                profile_raw = _builtin_fetch(_profiles_url(study_id), self.config.timeout_ms) if self._fetch is None else self._fetch(_profiles_url(study_id))
            except ReviewedCBioPortalRetrievalError:
                raise
            except Exception as error:
                raise ReviewedCBioPortalRetrievalError("cBioPortal metadata request failed") from error
            profile_response, profile_bytes = _parse_response(profile_raw)
            response_bytes += profile_bytes
            if response_bytes > MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES:
                _fail("cBioPortal aggregate response bytes exceed the plan bound")
            profiles = _profiles(profile_response, study_id)
            if len(profiles) >= MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY:
                _fail("cBioPortal profile catalogue may exceed the single reviewed page")
            payload = {"study": study, "molecular_profiles": profiles}
            study_digest = content_digest(payload)
            source_id = study["source_id"]
            sources.append({"source_id": source_id, "authority": REVIEWED_CBIOPORTAL_AUTHORITY, "uri": _study_url(study_id), "profile_uri": _profiles_url(study_id), "retrieved_at": timestamp, "content_sha256": study_digest, "record_count": 1 + len(profiles), "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)})
            source_receipts.append({"schema": REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA, "lane": study["lane"], "source_id": source_id, "study_id": study_id, "content_digest": study_digest, "profile_count": len(profiles), "metadata_completeness": "complete" if study["metadata_completeness"] == "complete" and profiles else "unknown"})
            studies.append(study)
            molecular_profiles.extend(profiles)
        completeness = "complete" if all(study["metadata_completeness"] == "complete" for study in studies) and bool(molecular_profiles) else "unknown"
        source_set_digest = content_digest(sources)
        bundle_unsigned = {"schema": REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA, "generated_at": timestamp, "sources": sources, "studies": studies, "molecular_profiles": molecular_profiles, "source_set_digest": source_set_digest, "study_count": len(studies), "profile_count": len(molecular_profiles), "completeness": completeness, "provider": "none", "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        _json_bytes(bundle_unsigned, "CBIOPORTAL bundle", MAX_REVIEWED_CBIOPORTAL_BUNDLE_BYTES)
        bundle = {**bundle_unsigned, "bundle_digest": content_digest(bundle_unsigned)}
        receipt_unsigned = {"schema": REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA, "plan_digest": plan.plan_digest, "config_digest": plan.config_digest, "study_set_digest": plan.study_set_digest, "bundle_digest": bundle["bundle_digest"], "source_set_digest": source_set_digest, "source_count": len(sources), "study_count": len(studies), "profile_count": len(molecular_profiles), "request_count": 2 * len(studies), "response_bytes": response_bytes, "completeness": completeness, "retrieved_at": timestamp, "source_receipts": source_receipts, "provider": "none", "network": "builtin_https" if self._fetch is None else "caller_transport", "effect": "read_only", "retention": _RETENTION, "credentials": "not_accepted", "limitations": list(_LIMITATIONS)}
        receipt = {**receipt_unsigned, "receipt_digest": content_digest(receipt_unsigned)}
        return ReviewedCBioPortalRetrievalResult(bundle, receipt)


def create_reviewed_cbioportal_execution_metadata(plan: ReviewedCBioPortalRetrievalPlan, *, approve_source_dispatch: bool, retrieved_at: str | None = None) -> dict[str, Any]:
    if type(plan) is not ReviewedCBioPortalRetrievalPlan:
        _fail("CBIOPORTAL execution metadata requires an exact plan")
    plan.validate()
    if approve_source_dispatch is not True:
        _fail("CBIOPORTAL execution metadata requires literal approval")
    timestamp = None if retrieved_at is None else _timestamp(retrieved_at, "CBIOPORTAL retrieved_at")
    payload = {"schema": REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA, "reviewed_plan_digest": plan.plan_digest, "approve_source_dispatch": True, "retrieved_at": timestamp, "retention": "metadata_only", "credentials": "not_accepted"}
    return {**payload, "metadata_digest": content_digest(payload)}


def _validate_transient(value: Any, plan: ReviewedCBioPortalRetrievalPlan, expected_network: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    if not isinstance(value, Mapping) or set(value) != {"schema", "bundle", "receipt", "retention"} or value.get("schema") != REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA or value.get("retention") != "caller_owned_transient_study_profile_metadata":
        _fail("CBIOPORTAL transient value is malformed")
    bundle, receipt = value["bundle"], value["receipt"]
    if not isinstance(bundle, Mapping) or not isinstance(receipt, Mapping):
        _fail("CBIOPORTAL transient bundle or receipt is malformed")
    bundle_keys = {"schema", "generated_at", "sources", "studies", "molecular_profiles", "source_set_digest", "study_count", "profile_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"}
    receipt_keys = {"schema", "plan_digest", "config_digest", "study_set_digest", "bundle_digest", "source_set_digest", "source_count", "study_count", "profile_count", "request_count", "response_bytes", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"}
    if set(bundle) != bundle_keys or set(receipt) != receipt_keys:
        _fail("CBIOPORTAL transient bundle or receipt has an unexpected shape")
    bundle_unsigned = {key: item for key, item in bundle.items() if key != "bundle_digest"}
    receipt_unsigned = {key: item for key, item in receipt.items() if key != "receipt_digest"}
    if content_digest(bundle_unsigned) != _digest("CBIOPORTAL bundle_digest", bundle.get("bundle_digest")) or content_digest(receipt_unsigned) != _digest("CBIOPORTAL receipt_digest", receipt.get("receipt_digest")):
        _fail("CBIOPORTAL transient digests are invalid")
    if bundle.get("schema") != REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA or receipt.get("schema") != REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA or receipt.get("plan_digest") != plan.plan_digest or receipt.get("config_digest") != plan.config_digest or receipt.get("study_set_digest") != plan.study_set_digest:
        _fail("CBIOPORTAL transient identity differs from the reviewed plan")
    if receipt.get("network") != expected_network or receipt.get("effect") != "read_only" or receipt.get("provider") != "none" or receipt.get("credentials") != "not_accepted" or receipt.get("retention") != _RETENTION or bundle.get("provider") != "none" or bundle.get("credentials") != "not_accepted":
        _fail("CBIOPORTAL transient boundary metadata is invalid")
    if canonical_json(bundle.get("limitations")) != canonical_json(list(_LIMITATIONS)) or canonical_json(receipt.get("limitations")) != canonical_json(list(_LIMITATIONS)):
        _fail("CBIOPORTAL transient limitations are invalid")
    timestamp = _timestamp(bundle.get("generated_at"), "CBIOPORTAL bundle timestamp")
    if timestamp != _timestamp(receipt.get("retrieved_at"), "CBIOPORTAL receipt timestamp"):
        _fail("CBIOPORTAL bundle and receipt timestamps do not match")
    studies, profiles, sources, source_receipts = bundle.get("studies"), bundle.get("molecular_profiles"), bundle.get("sources"), receipt.get("source_receipts")
    if not all(isinstance(rows, list) for rows in (studies, profiles, sources, source_receipts)) or len(studies) != len(plan.config.study_ids) or len(sources) != len(studies) or len(source_receipts) != len(studies) or len(profiles) > MAX_REVIEWED_CBIOPORTAL_STUDIES * MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY:
        _fail("CBIOPORTAL transient study coverage is incomplete")
    if any(not isinstance(profile, Mapping) or profile.get("study_id") not in plan.config.study_ids for profile in profiles):
        _fail("cBioPortal transient profiles contain an unreviewed study")
    if content_digest(sources) != bundle.get("source_set_digest") or receipt.get("source_set_digest") != bundle.get("source_set_digest") or receipt.get("bundle_digest") != bundle.get("bundle_digest"):
        _fail("CBIOPORTAL transient source binding is invalid")
    expected_profiles: list[dict[str, Any]] = []
    for study_id, study, source, source_receipt in zip(plan.config.study_ids, studies, sources, source_receipts):
        source_keys = {"source_id", "authority", "uri", "profile_uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"}
        source_receipt_keys = {"schema", "lane", "source_id", "study_id", "content_digest", "profile_count", "metadata_completeness"}
        if not isinstance(study, Mapping) or not isinstance(source, Mapping) or not isinstance(source_receipt, Mapping) or set(source) != source_keys or set(source_receipt) != source_receipt_keys:
            _fail("CBIOPORTAL transient study source metadata is malformed")
        study_payload = {"studyId": study_id, "name": study.get("name"), "description": study.get("description"), "cancerTypeId": study.get("cancer_type_id"), "publicStudy": study.get("public_study"), "pmid": study.get("pmid"), "allSampleCount": study.get("sample_count"), "referenceGenome": study.get("reference_genome"), "importDate": study.get("import_date")}
        normalized = _study(study_payload, study_id)
        study_profiles = [profile for profile in profiles if isinstance(profile, Mapping) and profile.get("study_id") == study_id]
        reconstructed_profiles = [{"studyId": study_id, "molecularProfileId": profile.get("profile_id"), "name": profile.get("name"), "molecularAlterationType": profile.get("molecular_alteration_type"), "genericAssayType": profile.get("generic_assay_type"), "datatype": profile.get("datatype"), "description": profile.get("description"), "showProfileInAnalysisTab": profile.get("show_in_analysis"), "patientLevel": profile.get("patient_level")} for profile in study_profiles]
        normalized_profiles = _profiles(reconstructed_profiles, study_id)
        if canonical_json(study_profiles) != canonical_json(normalized_profiles):
            _fail("cBioPortal transient molecular profiles are not normalized")
        expected_profiles.extend(normalized_profiles)
        if len(normalized_profiles) >= MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY:
            _fail("cBioPortal transient profile catalogue may be truncated")
        study_digest = content_digest({"study": normalized, "molecular_profiles": normalized_profiles})
        if canonical_json(study) != canonical_json(normalized) or source.get("source_id") != normalized["source_id"] or source.get("authority") != REVIEWED_CBIOPORTAL_AUTHORITY or source.get("uri") != _study_url(study_id) or source.get("profile_uri") != _profiles_url(study_id) or source.get("retrieved_at") != timestamp or source.get("content_sha256") != study_digest or source.get("record_count") != 1 + len(normalized_profiles) or source.get("provider") != "none" or source.get("credentials") != "not_accepted" or source.get("limitations") != list(_LIMITATIONS):
            _fail("CBIOPORTAL source metadata does not match its study")
        profile_completeness = "complete" if normalized["metadata_completeness"] == "complete" and normalized_profiles else "unknown"
        expected_receipt = {"schema": REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA, "lane": normalized["lane"], "source_id": normalized["source_id"], "study_id": study_id, "content_digest": study_digest, "profile_count": len(normalized_profiles), "metadata_completeness": profile_completeness}
        if canonical_json(source_receipt) != canonical_json(expected_receipt):
            _fail("CBIOPORTAL source receipt does not match its study")
    if canonical_json(profiles) != canonical_json(expected_profiles):
        _fail("cBioPortal transient profile order or coverage is invalid")
    for name, record, key, expected in (("cBioPortal request_count", receipt, "request_count", 2 * len(studies)), ("cBioPortal source_count", receipt, "source_count", len(studies)), ("cBioPortal study_count", receipt, "study_count", len(studies)), ("cBioPortal bundle study_count", bundle, "study_count", len(studies)), ("cBioPortal receipt profile_count", receipt, "profile_count", len(profiles)), ("cBioPortal bundle profile_count", bundle, "profile_count", len(profiles))):
        _integer(name, record.get(key), expected, expected)
    if receipt.get("request_count") != 2 * len(studies) or receipt.get("source_count") != len(studies) or receipt.get("study_count") != len(studies) or bundle.get("study_count") != len(studies) or receipt.get("profile_count") != len(profiles) or bundle.get("profile_count") != len(profiles):
        _fail("CBIOPORTAL transient counts are inconsistent")
    _integer("cBioPortal response_bytes", receipt.get("response_bytes"), 2 * len(studies), MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES)
    expected_completeness = "complete" if all(source_receipt.get("metadata_completeness") == "complete" for source_receipt in source_receipts) else "unknown"
    if receipt.get("completeness") != expected_completeness or bundle.get("completeness") != expected_completeness:
        _fail("CBIOPORTAL transient completeness is inconsistent")
    return bundle, receipt


def create_reviewed_cbioportal_autonomous_evidence_registration(adapter: ReviewedCBioPortalRetrievalAdapter, plan: ReviewedCBioPortalRetrievalPlan, *, study_id: str) -> AutonomousEvidenceAdapterRegistration:
    if type(adapter) is not ReviewedCBioPortalRetrievalAdapter or type(plan) is not ReviewedCBioPortalRetrievalPlan:
        _fail("CBIOPORTAL registration requires exact adapter and plan values")
    plan.validate()
    if not isinstance(study_id, str) or adapter.config != plan.config or study_id not in plan.config.study_ids or plan.config.study_ids != (study_id,):
        _fail("CBIOPORTAL registration requires the exact single-study plan")
    frozen_plan = ReviewedCBioPortalRetrievalPlan.create(plan.config)
    lane = _STUDY_TO_LANE[study_id]
    source_id = f"cbioportal_{lane}"
    expected_network = "builtin_https" if adapter._fetch is None else "caller_transport"

    def acquire(context: Mapping[str, Any]) -> dict[str, Any]:
        request = context.get("request") if isinstance(context, Mapping) else None
        if not isinstance(request, Mapping) or request.get("source_id") != source_id or request.get("source_digest") != frozen_plan.plan_digest:
            _fail("CBIOPORTAL acquisition request does not match its reviewed source")
        metadata = request.get("metadata")
        expected = {"schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"}
        if not isinstance(metadata, Mapping) or set(metadata) != expected:
            _fail("CBIOPORTAL acquisition metadata is malformed")
        unsigned = dict(metadata)
        supplied = _digest("CBIOPORTAL metadata_digest", unsigned.pop("metadata_digest"))
        if supplied != content_digest(unsigned) or metadata.get("schema") != REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA or metadata.get("reviewed_plan_digest") != frozen_plan.plan_digest or metadata.get("approve_source_dispatch") is not True or metadata.get("retention") != "metadata_only" or metadata.get("credentials") != "not_accepted":
            _fail("CBIOPORTAL acquisition metadata failed review binding")
        retrieved_at = None if metadata["retrieved_at"] is None else _timestamp(metadata["retrieved_at"], "CBIOPORTAL retrieved_at")
        return adapter.execute(frozen_plan, approve_source_dispatch=True, retrieved_at=retrieved_at).to_transient_dict()

    def project(value: Any, context: Mapping[str, Any]) -> list[dict[str, Any]]:
        _bundle, receipt = _validate_transient(value, frozen_plan, expected_network)
        requirement = context.get("requirement") if isinstance(context, Mapping) else None
        label = requirement.get("label") if isinstance(requirement, Mapping) else None
        if not isinstance(label, str) or not label.strip():
            _fail("CBIOPORTAL projection has no requirement label")
        return [{"label": label, "kind": "provenance", "status": "observed", "value_digest": receipt["bundle_digest"], "source_digest": receipt["source_set_digest"], "confidence": None, "limitations": list(_LIMITATIONS)}]

    return AutonomousEvidenceAdapterRegistration(adapter_id=f"reviewed.cbioportal.{lane}", version=REVIEWED_CBIOPORTAL_ADAPTER_VERSION, domains=("biomedical", "neuroscience"), capabilities=("aggregate_cohort_landscape", "source_provenance"), source_kinds=("cbioportal_public_study_profile_metadata",), acquire=acquire, project=project)


__all__ = [name for name in globals() if name.startswith("REVIEWED_CBIOPORTAL_") or name.startswith("MAX_REVIEWED_CBIOPORTAL_") or name.startswith("BUILTIN_CBIOPORTAL_")] + [
    "CbioPortalFetcher", "ReviewedCBioPortalRetrievalError", "ReviewedCBioPortalRetrievalConfig", "ReviewedCBioPortalRetrievalPlan", "ReviewedCBioPortalRetrievalResult", "ReviewedCBioPortalRetrievalAdapter", "create_reviewed_cbioportal_execution_metadata", "create_reviewed_cbioportal_autonomous_evidence_registration",
]
