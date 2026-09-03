"""Credentialless refresh of the public glioma population snapshot.

The Rust neurosurgery crate is the authoritative validator and query engine.  This module is the
portable ingestion edge for the compact ``bioprism-neurosurgery-real/0.1`` bundle: it retrieves
aggregate records from ClinicalTrials.gov, NCI GDC, cBioPortal, NCI PDQ, and PubMed, computes the
same canonical source/bundle digests as Rust, validates the candidate, and atomically installs it.

Only public metadata is retained.  The refresher never downloads patient records, assay values,
imaging, credentials, or synthetic fixtures, and it has no OpenAI/provider dependency.  ``fetch``
is injectable so the full parser and digest contract can be tested offline.  Partial PubMed
publication chronology is retained as missing rather than filled with an invented day.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
from typing import Any
from urllib.parse import quote, urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener
import xml.etree.ElementTree as ET


REAL_DATA_SCHEMA_VERSION = "bioprism-neurosurgery-real/0.1"
CLINICAL_TRIALS_AUTHORITY = "ClinicalTrials.gov / U.S. National Library of Medicine"
GDC_AUTHORITY = "NCI Genomic Data Commons"
CBIOPORTAL_AUTHORITY = "cBioPortal for Cancer Genomics"
NCI_AUTHORITY = "National Cancer Institute"
PUBMED_AUTHORITY = "U.S. National Library of Medicine PubMed"

CLINICAL_TRIALS_BASE = "https://clinicaltrials.gov/"
GDC_BASE = "https://api.gdc.cancer.gov/"
CBIOPORTAL_BASE = "https://www.cbioportal.org/"
NCI_BASE = "https://www.cancer.gov/"
PUBMED_EUTILS_BASE = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/"

MAX_REAL_SOURCES = 32
MAX_REAL_RECORDS = 4_096
MAX_GDC_PROJECTS = 16
MAX_TRIAL_PAGE_SIZE = 100
MAX_PORTAL_STUDIES = 100
MAX_PUBMED_RECORDS = 50
MAX_RESPONSE_BYTES = 16_000_000
MAX_TEXT_BYTES = 16_000
MAX_ABSTRACT_BYTES = 12_000
MAX_TAGS = 64
MAX_INTERVENTIONS = 128
MAX_GDC_DATA_TYPES = 256
MAX_GDC_FILE_COUNT = 100_000_000
MAX_ENROLLMENT = 10_000_000

DEFAULT_GDC_PROJECT_IDS = ("TCGA-GBM",)
DEFAULT_PORTAL_STUDY_IDS = (
    "gbm_mayo_pdx_sarkaria_2019",
    "gbm_cptac_2021",
    "gbm_columbia_2019",
    "gbm_iatlas_prins_2019",
    "gbm_tcga_pub2013",
    "gbm_tcga_pub",
    "gbm_tcga_gdc",
)
DEFAULT_PUBMED_TERM = "glioblastoma AND (molecular OR genomic)"
DEFAULT_PUBMED_SOURCE_ID = "pubmed_glioblastoma"
CLINICAL_TRIALS_QUERY = "Glioblastoma"
CBIOPORTAL_QUERY_URI = f"{CBIOPORTAL_BASE}api/studies?keyword=gbm"
NCI_PDQ_URI = f"{NCI_BASE}types/brain/hp/adult-brain-treatment-pdq"

_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")
_SYNTHETIC_RE = re.compile(
    r"(?:synthetic|fake\s+(?:patient|cohort|fixture)|generated\s+fixture)", re.IGNORECASE
)
_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_PROJECT_RE = re.compile(r"^TCGA-[A-Z0-9-]+$")
_SOURCE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9_-]{2,63}$")


class RealDataRefreshError(ValueError):
    """A public response or candidate glioma snapshot failed its bounded contract."""


RealDataFetcher = Callable[[str], bytes | str | Mapping[str, Any] | list[Any]]


@dataclass(frozen=True)
class RealDataRefreshReport:
    """Operator-safe refresh result; the snapshot itself remains caller-owned JSON."""

    schema_version: str
    bundle_digest: str
    generated_at: str
    source_count: int
    record_count: int
    clinical_trial_count: int
    genomic_project_count: int
    portal_study_count: int
    molecular_profile_count: int
    reference_count: int
    literature_count: int
    output_path: str | None = None
    network: bool = True
    synthetic_data: bool = False
    human_review_required: bool = True
    limitations: tuple[str, ...] = (
        "public registry and literature metadata are coverage inventory, not patient findings",
        "aggregate case/sample counts are source-reported and never establish eligibility or outcome",
        "a qualified reviewer must inspect freshness, omissions, study quality, and applicability",
    )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "bundle_digest": self.bundle_digest,
            "generated_at": self.generated_at,
            "source_count": self.source_count,
            "record_count": self.record_count,
            "clinical_trial_count": self.clinical_trial_count,
            "genomic_project_count": self.genomic_project_count,
            "portal_study_count": self.portal_study_count,
            "molecular_profile_count": self.molecular_profile_count,
            "reference_count": self.reference_count,
            "literature_count": self.literature_count,
            "output_path": self.output_path,
            "network": self.network,
            "synthetic_data": self.synthetic_data,
            "human_review_required": self.human_review_required,
            "limitations": list(self.limitations),
        }


def _text(value: Any, field: str, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    if value is None:
        raise RealDataRefreshError(f"{field} is missing")
    if not isinstance(value, str):
        value = str(value)
    value = " ".join(value.split())
    if required and not value:
        raise RealDataRefreshError(f"{field} is empty")
    if len(value.encode("utf-8")) > MAX_TEXT_BYTES or _CONTROL_RE.search(value):
        raise RealDataRefreshError(f"{field} exceeds the text safety bound")
    if _SYNTHETIC_RE.search(value):
        raise RealDataRefreshError(f"synthetic marker found in {field}")
    return value or None


def _decode(value: bytes | str | Mapping[str, Any] | list[Any], *, max_bytes: int = MAX_RESPONSE_BYTES) -> bytes | Mapping[str, Any] | list[Any]:
    if isinstance(value, (Mapping, list)):
        return value
    if isinstance(value, str):
        raw = value.encode("utf-8")
    elif isinstance(value, bytes):
        raw = value
    else:
        raise RealDataRefreshError("transport returned an unsupported response")
    if len(raw) > max_bytes:
        raise RealDataRefreshError("public response exceeds the bounded byte limit")
    return raw


def _json(value: bytes | str | Mapping[str, Any] | list[Any]) -> Mapping[str, Any] | list[Any]:
    decoded = _decode(value)
    if isinstance(decoded, (Mapping, list)):
        return decoded
    try:
        payload = json.loads(decoded.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RealDataRefreshError("public JSON response is malformed") from error
    if not isinstance(payload, (Mapping, list)):
        raise RealDataRefreshError("public JSON response must be an object or array")
    return payload


def _xml(value: bytes | str | Mapping[str, Any]) -> ET.Element:
    decoded = _decode(value)
    if isinstance(decoded, (Mapping, list)):
        raise RealDataRefreshError("PubMed XML response cannot be a mapping")
    try:
        return ET.fromstring(decoded)
    except (ET.ParseError, ValueError) as error:
        raise RealDataRefreshError("PubMed XML response is malformed") from error


class _AllowListedRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, request: Request, fp: Any, code: int, msg: str, headers: Any, newurl: str):
        if not _allowed_url(newurl):
            raise RealDataRefreshError("public endpoint redirected outside the allow-list")
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def _allowed_url(url: str) -> bool:
    parsed = urlsplit(url)
    if parsed.scheme != "https":
        return False
    if parsed.hostname == "clinicaltrials.gov":
        return parsed.path.startswith("/api/")
    if parsed.hostname == "api.gdc.cancer.gov":
        return parsed.path.startswith(("/projects/", "/cases", "/files"))
    if parsed.hostname == "www.cbioportal.org":
        return parsed.path.startswith("/api/")
    if parsed.hostname == "www.cancer.gov":
        return parsed.path.startswith("/")
    return parsed.hostname == "eutils.ncbi.nlm.nih.gov" and parsed.path.startswith("/entrez/eutils/")


def _default_fetch(url: str, *, timeout: float) -> bytes:
    if not _allowed_url(url):
        raise RealDataRefreshError("refresh attempted a non-allow-listed URL")
    request = Request(url, headers={"Accept": "application/json, application/xml", "User-Agent": "aurora-agent/0.1"})
    try:
        with build_opener(_AllowListedRedirectHandler()).open(request, timeout=timeout) as response:  # nosec B310 - URL is allow-listed above
            body = response.read(MAX_RESPONSE_BYTES + 1)
    except OSError as error:
        raise RealDataRefreshError("public endpoint request failed") from error
    if len(body) > MAX_RESPONSE_BYTES:
        raise RealDataRefreshError("public response exceeds the bounded byte limit")
    return body


def _utc_timestamp(value: str | None) -> str:
    if value is None:
        return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    if not isinstance(value, str) or not _UTC_RE.fullmatch(value):
        raise RealDataRefreshError("retrieved_at must be a UTC RFC3339 timestamp")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise RealDataRefreshError("retrieved_at is not a real UTC timestamp") from error
    return value


def _date(value: Any) -> str | None:
    if value is None:
        return None
    text = " ".join(str(value).split())
    if not text:
        return None
    if _DATE_RE.fullmatch(text):
        try:
            datetime.strptime(text, "%Y-%m-%d")
        except ValueError:
            return None
        return text
    # PubMed often reports only a year/month (for example ``2026 Jan``).  Do not
    # promote that partial chronology to ``YYYY-MM-01``: January 1 is not observed
    # by the source and would make inclusive date filters appear more precise than
    # the underlying metadata.  Only a complete day-bearing date is normalized.
    match = re.match(r"^(\d{4})\s+([A-Za-z]{3})\s+(\d{1,2})$", text)
    if match:
        months = {name: index for index, name in enumerate(("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"), 1)}
        month = months.get(match.group(2).title())
        try:
            return datetime(int(match.group(1)), month or 1, int(match.group(3) or 1)).strftime("%Y-%m-%d") if month else None
        except ValueError:
            return None
    # A year-only PubMed date is likewise retained as missing rather than assigned
    # an invented day.  The caller can still inspect the original source timestamp.
    return None


def _list(value: Any) -> list[Any]:
    if value is None:
        return []
    if isinstance(value, list):
        return value
    if isinstance(value, tuple):
        return list(value)
    return [value]


def _mapping(value: Any, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise RealDataRefreshError(f"{field} is not an object")
    return value


def _path(mapping: Mapping[str, Any], *keys: str) -> Any:
    current: Any = mapping
    for key in keys:
        if not isinstance(current, Mapping):
            return None
        current = current.get(key)
    return current


def _int_or_none(value: Any, field: str, *, maximum: int) -> int | None:
    if value is None or value == "":
        return None
    if isinstance(value, bool):
        raise RealDataRefreshError(f"{field} must be an integer")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise RealDataRefreshError(f"{field} must be an integer") from error
    if parsed < 0 or parsed > maximum:
        raise RealDataRefreshError(f"{field} is outside its bounded range")
    return parsed


def _unique_texts(values: Sequence[Any], field: str, *, maximum: int) -> list[str]:
    result: list[str] = []
    seen: set[str] = set()
    for value in values:
        text = _text(value, field, required=False)
        if text is not None and text not in seen:
            seen.add(text)
            result.append(text)
    if len(result) > maximum:
        raise RealDataRefreshError(f"{field} contains too many values")
    return result


def _url(path: str, **parameters: Any) -> str:
    return f"{path}?{urlencode(parameters, doseq=False, safe=',():') }"


def _pubmed_url(endpoint: str, **parameters: Any) -> str:
    if endpoint not in {"esearch.fcgi", "esummary.fcgi", "efetch.fcgi"}:
        raise RealDataRefreshError("unsupported PubMed endpoint")
    return _url(f"{PUBMED_EUTILS_BASE}{endpoint}", **parameters)


def _bounded_abstract(value: str | None) -> tuple[str | None, bool]:
    if value is None:
        return None, False
    encoded = value.encode("utf-8")
    if len(encoded) <= MAX_ABSTRACT_BYTES:
        return value, False
    return encoded[:MAX_ABSTRACT_BYTES].decode("utf-8", errors="ignore"), True


def _json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _canonical_record(kind: str, record: Mapping[str, Any]) -> dict[str, Any]:
    """Project a wire record using the Rust serde field order and skip-if-empty rules."""

    if kind == "clinical_trials":
        projected = {
            "source_id": record["source_id"],
            "nct_id": record["nct_id"],
            "title": record["title"],
            "overall_status": record["overall_status"],
            "phases": list(record.get("phases", [])),
            "last_update": record.get("last_update"),
        }
        for key in ("study_type", "enrollment_count"):
            if record.get(key) is not None:
                projected[key] = record[key]
        if record.get("intervention_names"):
            projected["intervention_names"] = list(record["intervention_names"])
        return projected
    if kind == "genomic_projects":
        projected = {
            "source_id": record["source_id"],
            "project_id": record["project_id"],
            "name": record["name"],
            "primary_site": list(record["primary_site"]),
            "disease_types": list(record["disease_types"]),
            "case_count": record["case_count"],
        }
        if record.get("data_type_counts"):
            projected["data_type_counts"] = [
                {"data_type": row["data_type"], "file_count": row["file_count"]}
                for row in record["data_type_counts"]
            ]
        return projected
    if kind == "portal_studies":
        return {
            "source_id": record["source_id"],
            "study_id": record["study_id"],
            "name": record["name"],
            "description": record["description"],
            "sample_count": record.get("sample_count"),
            "pmid": record.get("pmid"),
            "public_study": record["public_study"],
        }
    if kind == "portal_molecular_profiles":
        return {
            "source_id": record["source_id"],
            "study_id": record["study_id"],
            "profile_id": record["profile_id"],
            "name": record["name"],
            "molecular_alteration_type": record["molecular_alteration_type"],
            "datatype": record["datatype"],
            "description": record.get("description"),
            "show_in_analysis": record["show_in_analysis"],
            "patient_level": record["patient_level"],
        }
    if kind == "references":
        return {
            "source_id": record["source_id"],
            "reference_id": record["reference_id"],
            "title": record["title"],
            "uri": record["uri"],
            "publisher": record["publisher"],
        }
    if kind == "literature":
        projected = {
            "source_id": record["source_id"],
            "pmid": record["pmid"],
            "title": record["title"],
            "journal": record["journal"],
            "publication_date": record.get("publication_date"),
            "doi": record.get("doi"),
        }
        if record.get("abstract_text") is not None:
            projected["abstract_text"] = record["abstract_text"]
        if record.get("abstract_truncated", False):
            projected["abstract_truncated"] = True
        if record.get("publication_types"):
            projected["publication_types"] = list(record["publication_types"])
        if record.get("mesh_terms"):
            projected["mesh_terms"] = list(record["mesh_terms"])
        return projected
    raise RealDataRefreshError(f"unsupported canonical record kind {kind}")


def _source_content(bundle: Mapping[str, Any], source_id: str) -> dict[str, Any]:
    clinical = sorted((_canonical_record("clinical_trials", record) for record in bundle["clinical_trials"] if record["source_id"] == source_id), key=lambda record: record["nct_id"])
    genomic = sorted((_canonical_record("genomic_projects", record) for record in bundle["genomic_projects"] if record["source_id"] == source_id), key=lambda record: record["project_id"])
    portal = sorted((_canonical_record("portal_studies", record) for record in bundle["portal_studies"] if record["source_id"] == source_id), key=lambda record: record["study_id"])
    profiles = sorted((_canonical_record("portal_molecular_profiles", record) for record in bundle["portal_molecular_profiles"] if record["source_id"] == source_id), key=lambda record: (record["study_id"], record["profile_id"]))
    references = sorted((_canonical_record("references", record) for record in bundle["references"] if record["source_id"] == source_id), key=lambda record: record["reference_id"])
    literature = sorted((_canonical_record("literature", record) for record in bundle["literature"] if record["source_id"] == source_id), key=lambda record: record["pmid"])
    content: dict[str, Any] = {
        "clinical_trials": clinical,
        "genomic_projects": genomic,
        "portal_studies": portal,
    }
    if profiles:
        content["portal_molecular_profiles"] = profiles
    content["references"] = references
    if literature:
        content["literature"] = literature
    return content


def source_hash(bundle: Mapping[str, Any], source_id: str) -> str:
    return hashlib.sha256(_json_bytes(_source_content(bundle, source_id))).hexdigest()


def bundle_digest(bundle: Mapping[str, Any]) -> str:
    """Compute the digest of ``RealGliomaBundle::summary``'s serialized bundle."""

    projected = {
        "schema_version": bundle["schema_version"],
        "generated_at": bundle["generated_at"],
        "synthetic_data": bundle["synthetic_data"],
        "sources": bundle["sources"],
        "clinical_trials": [_canonical_record("clinical_trials", record) for record in bundle["clinical_trials"]],
        "genomic_projects": [_canonical_record("genomic_projects", record) for record in bundle["genomic_projects"]],
        "portal_studies": [_canonical_record("portal_studies", record) for record in bundle["portal_studies"]],
        "portal_molecular_profiles": [_canonical_record("portal_molecular_profiles", record) for record in bundle["portal_molecular_profiles"]],
        "references": [_canonical_record("references", record) for record in bundle["references"]],
        "literature": [_canonical_record("literature", record) for record in bundle["literature"]],
    }
    return hashlib.sha256(_json_bytes(projected)).hexdigest()


def _source_kind_ok(kind: str, uri: str) -> bool:
    prefixes = {
        "clinical_trials_registry": CLINICAL_TRIALS_BASE,
        "genomic_commons": GDC_BASE,
        "study_portal": CBIOPORTAL_BASE,
        "guideline": NCI_BASE,
        "literature_index": PUBMED_EUTILS_BASE,
    }
    return kind in prefixes and uri.startswith(prefixes[kind])


def validate_real_glioma_bundle(bundle: Mapping[str, Any]) -> None:
    """Validate the portable process-boundary subset of the Rust real-data contract."""

    if not isinstance(bundle, Mapping) or bundle.get("schema_version") != REAL_DATA_SCHEMA_VERSION:
        raise RealDataRefreshError("unsupported real-data schema")
    if bundle.get("synthetic_data") is not False:
        raise RealDataRefreshError("real-data snapshots require synthetic_data=false")
    generated_at = bundle.get("generated_at")
    if not isinstance(generated_at, str) or not _UTC_RE.fullmatch(generated_at):
        raise RealDataRefreshError("generated_at must be a UTC timestamp")
    try:
        datetime.strptime(generated_at, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise RealDataRefreshError("generated_at is not a real UTC timestamp") from error
    sources = bundle.get("sources")
    if not isinstance(sources, list) or not sources or len(sources) > MAX_REAL_SOURCES:
        raise RealDataRefreshError("real-data sources are outside bounds")
    source_by_id: dict[str, Mapping[str, Any]] = {}
    for source in sources:
        if not isinstance(source, Mapping):
            raise RealDataRefreshError("real-data source must be an object")
        source_id = _text(source.get("source_id"), "source.source_id")
        kind = _text(source.get("kind"), "source.kind")
        authority = _text(source.get("authority"), "source.authority")
        uri = _text(source.get("uri"), "source.uri")
        retrieved_at = _text(source.get("retrieved_at"), "source.retrieved_at")
        if source_id is None or kind is None or authority is None or uri is None or retrieved_at is None:
            raise RealDataRefreshError("real-data source is incomplete")
        if not _SOURCE_ID_RE.fullmatch(source_id) or source_id in source_by_id:
            raise RealDataRefreshError("real-data source identity is invalid or duplicated")
        if not _source_kind_ok(kind, uri) or not _UTC_RE.fullmatch(retrieved_at) or retrieved_at > generated_at:
            raise RealDataRefreshError("real-data source authority or timestamp is invalid")
        digest = source.get("content_sha256")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise RealDataRefreshError("real-data source digest is invalid")
        count = source.get("record_count")
        if isinstance(count, bool) or not isinstance(count, int) or count < 1:
            raise RealDataRefreshError("real-data source record_count is invalid")
        source_by_id[source_id] = source

    arrays = {
        "clinical_trials": bundle.get("clinical_trials"),
        "genomic_projects": bundle.get("genomic_projects"),
        "portal_studies": bundle.get("portal_studies"),
        "portal_molecular_profiles": bundle.get("portal_molecular_profiles", []),
        "references": bundle.get("references"),
        "literature": bundle.get("literature", []),
    }
    if any(not isinstance(value, list) for value in arrays.values()):
        raise RealDataRefreshError("real-data record arrays are malformed")
    if any(not arrays[key] for key in ("clinical_trials", "genomic_projects", "portal_studies", "references")):
        raise RealDataRefreshError("real-data bundle is missing a mandatory source plane")
    total_records = sum(len(value) for value in arrays.values())
    if total_records > MAX_REAL_RECORDS:
        raise RealDataRefreshError("real-data bundle exceeds its record bound")

    def record_source(record: Mapping[str, Any], field: str) -> str:
        source_id = _text(record.get("source_id"), field)
        if source_id is None or source_id not in source_by_id:
            raise RealDataRefreshError(f"{field} references an unknown source")
        return source_id

    seen_trials: set[str] = set()
    for record in arrays["clinical_trials"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("clinical trial is malformed")
        source_id = record_source(record, "trial.source_id")
        if source_by_id[source_id]["kind"] != "clinical_trials_registry":
            raise RealDataRefreshError("clinical trial source kind is invalid")
        nct_id = _text(record.get("nct_id"), "trial.nct_id")
        _text(record.get("title"), "trial.title")
        _text(record.get("overall_status"), "trial.overall_status")
        if nct_id is None or not nct_id.startswith("NCT") or nct_id in seen_trials:
            raise RealDataRefreshError("clinical trial identifier is invalid or duplicated")
        seen_trials.add(nct_id)
        phases = record.get("phases", [])
        if not isinstance(phases, list) or len(phases) > 16:
            raise RealDataRefreshError("trial phases are outside bounds")
        for value in phases:
            _text(value, "trial.phase")
        if record.get("last_update") is not None and _date(record["last_update"]) != record["last_update"]:
            raise RealDataRefreshError("trial last_update is invalid")
        if record.get("study_type") is not None:
            _text(record["study_type"], "trial.study_type")
        _int_or_none(record.get("enrollment_count"), "trial.enrollment_count", maximum=MAX_ENROLLMENT)
        interventions = record.get("intervention_names", [])
        if not isinstance(interventions, list) or len(interventions) > MAX_INTERVENTIONS:
            raise RealDataRefreshError("trial interventions are outside bounds")
        for value in interventions:
            _text(value, "trial.intervention_name")

    seen_projects: set[str] = set()
    for record in arrays["genomic_projects"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("genomic project is malformed")
        source_id = record_source(record, "project.source_id")
        if source_by_id[source_id]["kind"] != "genomic_commons":
            raise RealDataRefreshError("genomic project source kind is invalid")
        project_id = _text(record.get("project_id"), "project.project_id")
        _text(record.get("name"), "project.name")
        if project_id is None or project_id in seen_projects:
            raise RealDataRefreshError("genomic project identifier is invalid or duplicated")
        seen_projects.add(project_id)
        if _int_or_none(record.get("case_count"), "project.case_count", maximum=MAX_GDC_FILE_COUNT) in (None, 0):
            raise RealDataRefreshError("genomic project case_count is missing")
        for field in ("primary_site", "disease_types"):
            values = record.get(field)
            if not isinstance(values, list) or not values:
                raise RealDataRefreshError(f"project {field} is missing")
            for value in values:
                _text(value, f"project.{field}")
        facets = record.get("data_type_counts", [])
        if not isinstance(facets, list) or len(facets) > MAX_GDC_DATA_TYPES:
            raise RealDataRefreshError("project data-type facets are outside bounds")
        seen_types: set[str] = set()
        for facet in facets:
            if not isinstance(facet, Mapping):
                raise RealDataRefreshError("project data-type facet is malformed")
            data_type = _text(facet.get("data_type"), "project.data_type")
            count = _int_or_none(facet.get("file_count"), "project.file_count", maximum=MAX_GDC_FILE_COUNT)
            if data_type is None or count in (None, 0) or data_type in seen_types:
                raise RealDataRefreshError("project data-type facet is invalid")
            seen_types.add(data_type)

    seen_studies: set[str] = set()
    for record in arrays["portal_studies"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("portal study is malformed")
        source_id = record_source(record, "study.source_id")
        if source_by_id[source_id]["kind"] != "study_portal":
            raise RealDataRefreshError("portal study source kind is invalid")
        study_id = _text(record.get("study_id"), "study.study_id")
        _text(record.get("name"), "study.name")
        _text(record.get("description"), "study.description")
        if study_id is None or study_id in seen_studies or record.get("public_study") is not True:
            raise RealDataRefreshError("portal study identity is invalid")
        seen_studies.add(study_id)
        _int_or_none(record.get("sample_count"), "study.sample_count", maximum=MAX_GDC_FILE_COUNT)
        pmid = record.get("pmid")
        if pmid is not None and (not isinstance(pmid, str) or not pmid.isdigit()):
            raise RealDataRefreshError("portal study PMID is invalid")

    seen_profiles: set[tuple[str, str]] = set()
    for record in arrays["portal_molecular_profiles"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("molecular profile is malformed")
        source_id = record_source(record, "profile.source_id")
        if source_by_id[source_id]["kind"] != "study_portal":
            raise RealDataRefreshError("molecular profile source kind is invalid")
        study_id = _text(record.get("study_id"), "profile.study_id")
        profile_id = _text(record.get("profile_id"), "profile.profile_id")
        _text(record.get("name"), "profile.name")
        _text(record.get("molecular_alteration_type"), "profile.molecular_alteration_type")
        _text(record.get("datatype"), "profile.datatype")
        if study_id is None or profile_id is None or study_id not in seen_studies or (study_id, profile_id) in seen_profiles:
            raise RealDataRefreshError("molecular profile identity is invalid")
        seen_profiles.add((study_id, profile_id))
        if record.get("description") is not None:
            _text(record["description"], "profile.description")

    seen_refs: set[str] = set()
    for record in arrays["references"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("guideline reference is malformed")
        source_id = record_source(record, "reference.source_id")
        if source_by_id[source_id]["kind"] != "guideline":
            raise RealDataRefreshError("guideline source kind is invalid")
        reference_id = _text(record.get("reference_id"), "reference.reference_id")
        _text(record.get("title"), "reference.title")
        uri = _text(record.get("uri"), "reference.uri")
        _text(record.get("publisher"), "reference.publisher")
        if reference_id is None or uri is None or reference_id in seen_refs or not uri.startswith(NCI_BASE):
            raise RealDataRefreshError("guideline reference identity is invalid")
        seen_refs.add(reference_id)

    seen_pmids: set[str] = set()
    for record in arrays["literature"]:
        if not isinstance(record, Mapping):
            raise RealDataRefreshError("literature record is malformed")
        source_id = record_source(record, "literature.source_id")
        if source_by_id[source_id]["kind"] != "literature_index":
            raise RealDataRefreshError("literature source kind is invalid")
        pmid = _text(record.get("pmid"), "literature.pmid")
        _text(record.get("title"), "literature.title")
        _text(record.get("journal"), "literature.journal")
        if pmid is None or not pmid.isdigit() or pmid in seen_pmids:
            raise RealDataRefreshError("literature PMID is invalid or duplicated")
        seen_pmids.add(pmid)
        if record.get("publication_date") is not None and _date(record["publication_date"]) != record["publication_date"]:
            raise RealDataRefreshError("literature publication date is invalid")
        if record.get("doi") is not None and (not isinstance(record["doi"], str) or not record["doi"].startswith("10.")):
            raise RealDataRefreshError("literature DOI is invalid")
        abstract = record.get("abstract_text")
        if abstract is not None:
            _text(abstract, "literature.abstract_text")
            if len(abstract.encode("utf-8")) > MAX_ABSTRACT_BYTES:
                raise RealDataRefreshError("literature abstract is too long")
        if record.get("abstract_truncated", False) and abstract is None:
            raise RealDataRefreshError("literature abstract truncation is inconsistent")
        for field in ("publication_types", "mesh_terms"):
            values = record.get(field, [])
            if not isinstance(values, list) or len(values) > MAX_TAGS:
                raise RealDataRefreshError(f"literature {field} is outside bounds")
            for value in values:
                _text(value, f"literature.{field}")

    mandatory_kinds = {"clinical_trials_registry", "genomic_commons", "study_portal", "guideline"}
    if mandatory_kinds - {str(source["kind"]) for source in sources}:
        raise RealDataRefreshError("real-data bundle lacks a mandatory source kind")
    for source_id, source in source_by_id.items():
        content = _source_content(bundle, source_id)
        count = sum(len(value) for value in content.values())
        if count != source["record_count"] or source_hash(bundle, source_id) != source["content_sha256"]:
            raise RealDataRefreshError(f"source {source_id} hash or record count is invalid")


def _summary(bundle: Mapping[str, Any], *, output_path: str | None = None, network: bool = True) -> RealDataRefreshReport:
    arrays = {
        key: bundle[key]
        for key in (
            "clinical_trials",
            "genomic_projects",
            "portal_studies",
            "portal_molecular_profiles",
            "references",
            "literature",
        )
    }
    return RealDataRefreshReport(
        schema_version=REAL_DATA_SCHEMA_VERSION,
        bundle_digest=bundle_digest(bundle),
        generated_at=str(bundle["generated_at"]),
        source_count=len(bundle["sources"]),
        record_count=sum(len(value) for value in arrays.values()),
        clinical_trial_count=len(arrays["clinical_trials"]),
        genomic_project_count=len(arrays["genomic_projects"]),
        portal_study_count=len(arrays["portal_studies"]),
        molecular_profile_count=len(arrays["portal_molecular_profiles"]),
        reference_count=len(arrays["references"]),
        literature_count=len(arrays["literature"]),
        output_path=output_path,
        network=network,
    )


def _fetch_once(fetch: RealDataFetcher, url: str) -> bytes | str | Mapping[str, Any] | list[Any]:
    try:
        return fetch(url)
    except RealDataRefreshError:
        raise
    except Exception as error:
        raise RealDataRefreshError("public transport failed") from error


def _parse_trials(payload: Mapping[str, Any], *, limit: int) -> list[dict[str, Any]]:
    studies = payload.get("studies")
    if not isinstance(studies, list):
        raise RealDataRefreshError("ClinicalTrials.gov response has no studies array")
    records: list[dict[str, Any]] = []
    for study in studies[:limit]:
        root = _mapping(study, "clinical trial")
        identification = _mapping(_path(root, "protocolSection", "identificationModule"), "trial identificationModule")
        status = _mapping(_path(root, "protocolSection", "statusModule"), "trial statusModule")
        design = _mapping(_path(root, "protocolSection", "designModule") or {}, "trial designModule")
        nct_id = _text(identification.get("nctId"), "trial.nctId")
        title = _text(identification.get("briefTitle") or identification.get("officialTitle"), "trial.title")
        overall_status = _text(status.get("overallStatus"), "trial.overallStatus")
        if nct_id is None or title is None or overall_status is None:
            raise RealDataRefreshError("ClinicalTrials.gov trial is incomplete")
        phases = _unique_texts(_list(design.get("phases")), "trial.phases", maximum=16)
        last_update = _date(_path(status, "lastUpdatePostDateStruct", "date"))
        study_type = _text(design.get("studyType"), "trial.studyType", required=False)
        enrollment = _int_or_none(_path(design, "enrollmentInfo", "count"), "trial.enrollmentCount", maximum=MAX_ENROLLMENT)
        interventions = _path(root, "protocolSection", "armsInterventionsModule", "interventions")
        names = _unique_texts([_path(_mapping(item, "intervention"), "name") for item in _list(interventions)], "trial.interventionNames", maximum=MAX_INTERVENTIONS)
        record: dict[str, Any] = {
            "source_id": "clinicaltrials_glioblastoma",
            "nct_id": nct_id,
            "title": title,
            "overall_status": overall_status,
            "phases": phases,
            "last_update": last_update,
        }
        if study_type is not None:
            record["study_type"] = study_type
        if enrollment is not None:
            record["enrollment_count"] = enrollment
        if names:
            record["intervention_names"] = names
        records.append(record)
    if not records:
        raise RealDataRefreshError("ClinicalTrials.gov returned no usable studies")
    return records


def _parse_gdc_project(project: Mapping[str, Any], cases: Mapping[str, Any], files: Mapping[str, Any], project_id: str) -> dict[str, Any]:
    data = _mapping(project.get("data"), "GDC project data")
    actual_id = _text(data.get("project_id"), "GDC project_id")
    name = _text(data.get("name"), "GDC project name")
    if actual_id is None or name is None or actual_id != project_id:
        raise RealDataRefreshError("GDC project identity does not match the requested project")
    pagination = _mapping(_path(cases, "data", "pagination"), "GDC case pagination")
    case_count = _int_or_none(pagination.get("total"), "GDC case count", maximum=MAX_GDC_FILE_COUNT)
    primary_site = _unique_texts(_list(data.get("primary_site")), "GDC primary_site", maximum=32)
    disease_types = _unique_texts(_list(data.get("disease_type")), "GDC disease_type", maximum=32)
    buckets = _path(files, "data", "aggregations", "data_type", "buckets")
    if not isinstance(buckets, list):
        raise RealDataRefreshError("GDC file facet response is missing data_type buckets")
    facets: list[dict[str, Any]] = []
    for bucket in buckets:
        row = _mapping(bucket, "GDC data-type bucket")
        data_type = _text(row.get("key"), "GDC data_type")
        count = _int_or_none(row.get("doc_count"), "GDC data_type file_count", maximum=MAX_GDC_FILE_COUNT)
        if data_type is not None and count not in (None, 0):
            facets.append({"data_type": data_type, "file_count": count})
    facets.sort(key=lambda row: row["data_type"])
    if case_count in (None, 0) or not primary_site or not disease_types or not facets:
        raise RealDataRefreshError("GDC project is missing aggregate coverage metadata")
    return {
        "source_id": f"gdc_{project_id.lower().replace('-', '_')}",
        "project_id": actual_id,
        "name": name,
        "primary_site": primary_site,
        "disease_types": disease_types,
        "case_count": case_count,
        "data_type_counts": facets,
    }


def _parse_pubmed(ids: list[str], summary: Mapping[str, Any], xml_root: ET.Element, source_id: str) -> list[dict[str, Any]]:
    summary_result = summary.get("result")
    if not isinstance(summary_result, Mapping):
        raise RealDataRefreshError("PubMed summary response has no result object")
    xml_content: dict[str, dict[str, Any]] = {}
    for article in xml_root.findall(".//PubmedArticle"):
        pmid_node = article.find("./MedlineCitation/PMID")
        pmid = (pmid_node.text or "").strip() if pmid_node is not None else ""
        if not pmid:
            continue
        parts: list[str] = []
        for node in article.findall("./MedlineCitation/Article/Abstract/AbstractText"):
            text = " ".join("".join(node.itertext()).split())
            if text:
                label = (node.attrib.get("Label") or "").strip()
                parts.append(f"{label}: {text}" if label else text)
        abstract, truncated = _bounded_abstract(" ".join(parts) if parts else None)
        publication_types = [" ".join("".join(node.itertext()).split()) for node in article.findall("./MedlineCitation/Article/PublicationTypeList/PublicationType")]
        mesh_terms = [" ".join("".join(node.itertext()).split()) for node in article.findall("./MedlineCitation/MeshHeadingList/MeshHeading/DescriptorName")]
        xml_content[pmid] = {
            "abstract_text": abstract,
            "abstract_truncated": truncated,
            "publication_types": [value for value in publication_types if value],
            "mesh_terms": [value for value in mesh_terms if value],
        }
    records: list[dict[str, Any]] = []
    for pmid in ids:
        article = summary_result.get(pmid)
        if not isinstance(article, Mapping):
            continue
        title = _text(article.get("title"), f"PubMed {pmid} title")
        journal = _text(article.get("fulljournalname") or article.get("source"), f"PubMed {pmid} journal")
        if title is None or journal is None:
            continue
        doi = None
        for identifier in _list(article.get("articleids")):
            if isinstance(identifier, Mapping) and identifier.get("idtype") == "doi":
                doi = _text(identifier.get("value"), f"PubMed {pmid} doi", required=False)
                break
        content = xml_content.get(pmid, {})
        record: dict[str, Any] = {
            "source_id": source_id,
            "pmid": pmid,
            "title": title,
            "journal": journal,
            "publication_date": _date(article.get("epubdate") or article.get("pubdate")),
            "doi": doi,
        }
        if content.get("abstract_text") is not None:
            record["abstract_text"] = content["abstract_text"]
        if content.get("abstract_truncated"):
            record["abstract_truncated"] = True
        if content.get("publication_types"):
            record["publication_types"] = content["publication_types"]
        if content.get("mesh_terms"):
            record["mesh_terms"] = content["mesh_terms"]
        records.append(record)
    if not records:
        raise RealDataRefreshError("PubMed returned no usable glioma citations")
    return records


def refresh_real_glioma_data(
    *,
    fetch: RealDataFetcher | None = None,
    gdc_project_ids: Sequence[str] = DEFAULT_GDC_PROJECT_IDS,
    trial_page_size: int = 5,
    portal_study_ids: Sequence[str] = DEFAULT_PORTAL_STUDY_IDS,
    portal_study_limit: int = 7,
    pubmed_limit: int = 20,
    pubmed_term: str = DEFAULT_PUBMED_TERM,
    pubmed_source_id: str = DEFAULT_PUBMED_SOURCE_ID,
    retrieved_at: str | None = None,
    timeout: float = 30.0,
) -> tuple[dict[str, Any], RealDataRefreshReport]:
    """Retrieve and validate one bounded real glioma population snapshot."""

    projects = tuple(gdc_project_ids)
    study_ids = tuple(portal_study_ids)
    if not 1 <= len(projects) <= MAX_GDC_PROJECTS or any(not isinstance(value, str) or not _PROJECT_RE.fullmatch(value) for value in projects):
        raise RealDataRefreshError("gdc_project_ids must contain 1..16 TCGA project IDs")
    if not 1 <= trial_page_size <= MAX_TRIAL_PAGE_SIZE:
        raise RealDataRefreshError("trial_page_size is outside bounds")
    if not 1 <= len(study_ids) <= MAX_PORTAL_STUDIES or not 1 <= portal_study_limit <= MAX_PORTAL_STUDIES:
        raise RealDataRefreshError("portal study selection is outside bounds")
    if portal_study_limit > len(study_ids):
        raise RealDataRefreshError("portal_study_limit exceeds portal_study_ids")
    if not 1 <= pubmed_limit <= MAX_PUBMED_RECORDS:
        raise RealDataRefreshError("pubmed_limit is outside bounds")
    if not isinstance(pubmed_term, str) or not pubmed_term.strip() or len(pubmed_term.encode("utf-8")) > 512 or _CONTROL_RE.search(pubmed_term):
        raise RealDataRefreshError("pubmed_term is invalid")
    if not isinstance(pubmed_source_id, str) or not _SOURCE_ID_RE.fullmatch(pubmed_source_id):
        raise RealDataRefreshError("pubmed_source_id is invalid")
    if not isinstance(timeout, (int, float)) or isinstance(timeout, bool) or not 1 <= timeout <= 120:
        raise RealDataRefreshError("timeout is outside bounds")
    generated_at = _utc_timestamp(retrieved_at)
    network = fetch is None
    if fetch is None:
        def fetch(url: str) -> bytes:
            return _default_fetch(url, timeout=float(timeout))

    clinical_uri = _url(f"{CLINICAL_TRIALS_BASE}api/v2/studies", **{"query.cond": CLINICAL_TRIALS_QUERY, "pageSize": trial_page_size, "format": "json"})
    clinical_trials = _parse_trials(_mapping(_json(_fetch_once(fetch, clinical_uri)), "ClinicalTrials.gov response"), limit=trial_page_size)

    genomic_projects: list[dict[str, Any]] = []
    genomic_sources: list[dict[str, Any]] = []
    for project_id in projects:
        project_uri = f"{GDC_BASE}projects/{project_id}?format=json"
        cases_filter = json.dumps({"op": "=", "content": {"field": "project.project_id", "value": project_id}}, separators=(",", ":"))
        files_filter = json.dumps({"op": "=", "content": {"field": "cases.project.project_id", "value": project_id}}, separators=(",", ":"))
        cases_uri = _url(f"{GDC_BASE}cases", filters=cases_filter, format="json", size=0)
        files_uri = _url(f"{GDC_BASE}files", filters=files_filter, facets="data_type", format="json", size=0)
        project = _mapping(_json(_fetch_once(fetch, project_uri)), "GDC project response")
        cases = _mapping(_json(_fetch_once(fetch, cases_uri)), "GDC case response")
        files = _mapping(_json(_fetch_once(fetch, files_uri)), "GDC file response")
        genomic_projects.append(_parse_gdc_project(project, cases, files, project_id))
        genomic_sources.append({
            "source_id": f"gdc_{project_id.lower().replace('-', '_')}",
            "kind": "genomic_commons",
            "authority": GDC_AUTHORITY,
            "uri": project_uri,
            "retrieved_at": generated_at,
            "content_sha256": "0" * 64,
            "record_count": 1,
        })

    portal_payload = _json(_fetch_once(fetch, CBIOPORTAL_QUERY_URI))
    if not isinstance(portal_payload, list):
        raise RealDataRefreshError("cBioPortal study response must be an array")
    portal_studies: list[dict[str, Any]] = []
    portal_profiles: list[dict[str, Any]] = []
    for study_id in study_ids[:portal_study_limit]:
        match = next((row for row in portal_payload if isinstance(row, Mapping) and row.get("studyId") == study_id and row.get("publicStudy") is True), None)
        if match is None:
            raise RealDataRefreshError(f"cBioPortal public study {study_id} was not present")
        description = _text(match.get("description"), f"cBioPortal {study_id} description")
        name = _text(match.get("name"), f"cBioPortal {study_id} name")
        if description is None or name is None:
            raise RealDataRefreshError("cBioPortal study is incomplete")
        # The study-level endpoint exposes an aggregate count.  Do not enumerate the samples
        # endpoint: its response contains sample/patient identifiers that this ingestion edge has
        # no reason to retrieve, even transiently.
        detail_uri = f"{CBIOPORTAL_BASE}api/studies/{quote(study_id, safe='')}"
        detail = _json(_fetch_once(fetch, detail_uri))
        detail_mapping = _mapping(detail, "cBioPortal study detail")
        sample_count = _int_or_none(detail_mapping.get("allSampleCount"), "cBioPortal sample_count", maximum=MAX_GDC_FILE_COUNT)
        pmid_raw = match.get("pmid")
        pmid = next((token for token in re.split(r"[,;\s]+", str(pmid_raw)) if token.isdigit()), None) if pmid_raw else None
        portal_studies.append({
            "source_id": "cbioportal_gbm_catalog",
            "study_id": study_id,
            "name": name,
            "description": description,
            "sample_count": sample_count,
            "pmid": pmid,
            "public_study": True,
        })
        profiles_uri = f"{CBIOPORTAL_BASE}api/studies/{quote(study_id, safe='')}/molecular-profiles"
        profiles = _json(_fetch_once(fetch, profiles_uri))
        if not isinstance(profiles, list):
            raise RealDataRefreshError("cBioPortal molecular-profile response is not an array")
        for profile in profiles:
            row = _mapping(profile, "cBioPortal molecular profile")
            profile_id = _text(row.get("molecularProfileId"), "profile.molecularProfileId")
            profile_name = _text(row.get("name"), "profile.name")
            alteration = _text(row.get("molecularAlterationType"), "profile.molecularAlterationType")
            datatype = _text(row.get("datatype"), "profile.datatype")
            if None in (profile_id, profile_name, alteration, datatype):
                raise RealDataRefreshError("cBioPortal molecular profile is incomplete")
            portal_profiles.append({
                "source_id": "cbioportal_gbm_catalog",
                "study_id": study_id,
                "profile_id": profile_id,
                "name": profile_name,
                "molecular_alteration_type": alteration,
                "datatype": datatype,
                "description": _text(row.get("description"), "profile.description", required=False),
                "show_in_analysis": bool(row.get("showProfileInAnalysisTab", False)),
                "patient_level": bool(row.get("patientLevel", False)),
            })
    if not portal_studies or not portal_profiles:
        raise RealDataRefreshError("cBioPortal returned no usable study/profile metadata")

    pubmed_search_uri = _pubmed_url("esearch.fcgi", db="pubmed", term=pubmed_term, retmax=pubmed_limit, retmode="json", sort="pub_date")
    search = _mapping(_json(_fetch_once(fetch, pubmed_search_uri)), "PubMed search response")
    result = _mapping(search.get("esearchresult"), "PubMed search result")
    ids = [str(value) for value in _list(result.get("idlist")) if str(value).isdigit()][:pubmed_limit]
    if not ids:
        raise RealDataRefreshError("PubMed returned no valid glioma PMIDs")
    joined = ",".join(ids)
    summary_uri = _pubmed_url("esummary.fcgi", db="pubmed", id=joined, retmode="json")
    fetch_uri = _pubmed_url("efetch.fcgi", db="pubmed", id=joined, rettype="abstract", retmode="xml")
    literature = _parse_pubmed(ids, _mapping(_json(_fetch_once(fetch, summary_uri)), "PubMed summary"), _xml(_fetch_once(fetch, fetch_uri)), pubmed_source_id)

    sources: list[dict[str, Any]] = [{
        "source_id": "clinicaltrials_glioblastoma",
        "kind": "clinical_trials_registry",
        "authority": CLINICAL_TRIALS_AUTHORITY,
        "uri": clinical_uri,
        "retrieved_at": generated_at,
        "content_sha256": "0" * 64,
        "record_count": len(clinical_trials),
    }]
    sources.extend(genomic_sources)
    sources.extend([
        {
            "source_id": "cbioportal_gbm_catalog",
            "kind": "study_portal",
            "authority": CBIOPORTAL_AUTHORITY,
            "uri": CBIOPORTAL_QUERY_URI,
            "retrieved_at": generated_at,
            "content_sha256": "0" * 64,
            "record_count": len(portal_studies) + len(portal_profiles),
        },
        {
            "source_id": "nci_adult_cns_pdq",
            "kind": "guideline",
            "authority": NCI_AUTHORITY,
            "uri": NCI_PDQ_URI,
            "retrieved_at": generated_at,
            "content_sha256": "0" * 64,
            "record_count": 1,
        },
        {
            "source_id": pubmed_source_id,
            "kind": "literature_index",
            "authority": PUBMED_AUTHORITY,
            "uri": pubmed_search_uri,
            "retrieved_at": generated_at,
            "content_sha256": "0" * 64,
            "record_count": len(literature),
        },
    ])
    bundle: dict[str, Any] = {
        "schema_version": REAL_DATA_SCHEMA_VERSION,
        "generated_at": generated_at,
        "synthetic_data": False,
        "sources": sources,
        "clinical_trials": clinical_trials,
        "genomic_projects": genomic_projects,
        "portal_studies": portal_studies,
        "portal_molecular_profiles": portal_profiles,
        "references": [{
            "source_id": "nci_adult_cns_pdq",
            "reference_id": "NCI-PDQ-adult-CNS",
            "title": "Central Nervous System Tumors Treatment (PDQ) - Health Professional Version",
            "uri": NCI_PDQ_URI,
            "publisher": NCI_AUTHORITY,
        }],
        "literature": literature,
    }
    for source in bundle["sources"]:
        source["content_sha256"] = source_hash(bundle, source["source_id"])
    validate_real_glioma_bundle(bundle)
    return bundle, _summary(bundle, network=network)


def atomic_refresh_real_glioma_data(
    output_path: str | os.PathLike[str],
    **kwargs: Any,
) -> RealDataRefreshReport:
    """Install a validated candidate with same-directory atomic replacement."""

    destination = Path(output_path)
    if destination.exists() and not destination.is_file():
        raise RealDataRefreshError("refresh output path is not a file")
    destination.parent.mkdir(parents=True, exist_ok=True)
    bundle, report = refresh_real_glioma_data(**kwargs)
    candidate: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", newline="\n", prefix=f".{destination.name}.", suffix=".candidate", dir=destination.parent, delete=False) as handle:
            candidate = Path(handle.name)
            json.dump(bundle, handle, ensure_ascii=False, sort_keys=False, separators=(",", ":"))
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(candidate, destination)
    finally:
        if candidate is not None and candidate.exists():
            candidate.unlink(missing_ok=True)
    return RealDataRefreshReport(**{**report.__dict__, "output_path": str(destination)})


__all__ = [
    "REAL_DATA_SCHEMA_VERSION",
    "DEFAULT_GDC_PROJECT_IDS",
    "DEFAULT_PORTAL_STUDY_IDS",
    "DEFAULT_PUBMED_TERM",
    "DEFAULT_PUBMED_SOURCE_ID",
    "RealDataFetcher",
    "RealDataRefreshError",
    "RealDataRefreshReport",
    "source_hash",
    "bundle_digest",
    "validate_real_glioma_bundle",
    "refresh_real_glioma_data",
    "atomic_refresh_real_glioma_data",
]
