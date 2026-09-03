"""Bounded, credentialless PubMed refresh for the neurosurgical evidence plane.

The Rust neurosurgery crate remains the authoritative validator and query engine.  This module
owns only the network ingestion edge: it asks the public NCBI E-utilities endpoints for citation
metadata, builds the exact ``bioprism-neurosurgery-public-literature/0.1`` snapshot shape, checks
the same source hashes as Rust, and replaces an output file only after the candidate is valid.

There is deliberately no API-key parameter, provider integration, synthetic fallback, or patient
data path.  ``fetch`` is injectable so tests and offline callers can exercise parsing and digest
parity without a network request.  Partial publication chronology remains missing instead of
being padded with an invented day.
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
import time
from typing import Any
from urllib.parse import quote, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener
import xml.etree.ElementTree as ET


PUBLIC_LITERATURE_SCHEMA_VERSION = "bioprism-neurosurgery-public-literature/0.1"
PUBMED_AUTHORITY = "U.S. National Library of Medicine PubMed"
PUBMED_EUTILS_BASE = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/"
PUBMED_RECORD_BASE = "https://pubmed.ncbi.nlm.nih.gov/"
MAX_PUBMED_LANES = 6
MAX_SOURCE_COUNT = 32
MAX_RECORD_COUNT = 4_096
MAX_PER_SPECIALTY_LIMIT = 50
MAX_TOTAL_RECORDS = 300
MAX_RESPONSE_BYTES = 8_000_000
MAX_ABSTRACT_BYTES = 12_000
MAX_TAGS = 64
MAX_TEXT_BYTES = 16_000

# Keep this vocabulary in one place.  It is intentionally broad enough to retrieve specialist
# literature while retaining a stable lane identity for audits and replay.
PUBMED_SPECIALTY_LANES: dict[str, str] = {
    "glioma": (
        '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma") '
        'AND (molecular OR genomic OR pseudoprogression OR "radiation necrosis")'
    ),
    "cranial_base": (
        '((skull base) OR (cranial base) OR petroclival OR "cavernous sinus" OR "cranial nerve" '
        'OR "CSF leak") AND (neurosurgery OR surgery)'
    ),
    "craniosynostosis": (
        '(craniosynostosis OR scaphocephaly OR plagiocephaly OR "Apert syndrome" OR '
        '"Crouzon syndrome" OR "Pfeiffer syndrome")'
    ),
    "encephalocele": (
        '(encephalocele OR meningoencephalocele OR "basal encephalocele" OR '
        '"occipital encephalocele" OR "CSF rhinorrhea")'
    ),
    "spina_bifida": (
        "((spina bifida) OR (spinal dysraphism) OR myelomeningocele OR lipomeningocele OR "
        '"tethered cord" OR "neurogenic bladder" OR diastematomyelia)'
    ),
    "chiari_malformation": (
        '((Chiari malformation) OR (craniocervical junction) OR syringomyelia OR "cine MRI" '
        'OR "CSF flow" OR "clivo-axial angle" OR "basilar invagination")'
    ),
}

_MONTHS = {
    name: index
    for index, name in enumerate(
        (
            "Jan",
            "Feb",
            "Mar",
            "Apr",
            "May",
            "Jun",
            "Jul",
            "Aug",
            "Sep",
            "Oct",
            "Nov",
            "Dec",
        ),
        1,
    )
}
_SYNTHETIC_MARKERS = (
    "synthetic fixture",
    "synthetic case",
    "synthetic patient",
    "synthetic cohort",
    "generated fixture",
    "fake patient",
)
_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")
_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
_NCBI_TOOL_RE = re.compile(r"^[A-Za-z0-9_.:+-]{1,128}$")
_NCBI_EMAIL_RE = re.compile(
    r"^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]{1,64}"
    r"@(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?\.)+[A-Za-z]{2,63}$"
)


class PublicLiteratureRefreshError(ValueError):
    """A refresh response or candidate snapshot failed the bounded real-data contract."""


PubMedFetcher = Callable[[str], bytes | str | Mapping[str, Any]]


@dataclass(frozen=True)
class PublicLiteratureRefreshReport:
    """Operator-safe refresh result; the bundle itself remains caller-owned JSON."""

    schema_version: str
    bundle_digest: str
    generated_at: str
    source_count: int
    record_count: int
    abstract_count: int
    specialty_counts: dict[str, int]
    output_path: str | None = None
    network: bool = True
    synthetic_data: bool = False
    human_review_required: bool = True
    limitations: tuple[str, ...] = (
        "PubMed metadata and abstracts are source text, not verified clinical conclusions",
        "the snapshot is population-level literature and contains no patient record or clinical action",
        "a qualified reviewer must inspect freshness, omissions, study quality, and applicability",
    )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "bundle_digest": self.bundle_digest,
            "generated_at": self.generated_at,
            "source_count": self.source_count,
            "record_count": self.record_count,
            "abstract_count": self.abstract_count,
            "specialty_counts": dict(self.specialty_counts),
            "output_path": self.output_path,
            "network": self.network,
            "synthetic_data": self.synthetic_data,
            "human_review_required": self.human_review_required,
            "limitations": list(self.limitations),
        }


def _text(value: Any, field: str, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    if not isinstance(value, str):
        if value is None:
            raise PublicLiteratureRefreshError(f"{field} is missing")
        value = str(value)
    value = " ".join(value.split())
    if required and not value:
        raise PublicLiteratureRefreshError(f"{field} is empty")
    if len(value.encode("utf-8")) > MAX_TEXT_BYTES or _CONTROL_RE.search(value):
        raise PublicLiteratureRefreshError(f"{field} exceeds the text safety bound")
    if any(marker in value.casefold() for marker in _SYNTHETIC_MARKERS):
        raise PublicLiteratureRefreshError(f"synthetic marker found in {field}")
    return value or None


def _decode_response(
    value: bytes | str | Mapping[str, Any], *, max_bytes: int
) -> bytes | Mapping[str, Any]:
    if isinstance(value, Mapping):
        return value
    if isinstance(value, str):
        raw = value.encode("utf-8")
    elif isinstance(value, bytes):
        raw = value
    else:
        raise PublicLiteratureRefreshError(
            "PubMed transport returned an unsupported response"
        )
    if len(raw) > max_bytes:
        raise PublicLiteratureRefreshError(
            "PubMed response exceeds the bounded byte limit"
        )
    return raw


def _json_response(value: bytes | str | Mapping[str, Any]) -> Mapping[str, Any]:
    decoded = _decode_response(value, max_bytes=MAX_RESPONSE_BYTES)
    if isinstance(decoded, Mapping):
        payload = decoded
    else:
        try:
            payload = json.loads(decoded.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise PublicLiteratureRefreshError(
                "PubMed JSON response is malformed"
            ) from error
    if not isinstance(payload, Mapping):
        raise PublicLiteratureRefreshError("PubMed JSON response must be an object")
    return payload


def _xml_response(value: bytes | str | Mapping[str, Any]) -> ET.Element:
    decoded = _decode_response(value, max_bytes=MAX_RESPONSE_BYTES)
    if isinstance(decoded, Mapping):
        raise PublicLiteratureRefreshError("PubMed XML response cannot be a mapping")
    try:
        return ET.fromstring(decoded)
    except (ET.ParseError, ValueError) as error:
        raise PublicLiteratureRefreshError(
            "PubMed XML response is malformed"
        ) from error


class _AllowListedRedirectHandler(HTTPRedirectHandler):
    def redirect_request(
        self, request: Request, fp: Any, code: int, msg: str, headers: Any, newurl: str
    ):
        parsed = urlsplit(newurl)
        if (
            parsed.scheme != "https"
            or parsed.hostname != "eutils.ncbi.nlm.nih.gov"
            or not parsed.path.startswith("/entrez/eutils/")
        ):
            raise PublicLiteratureRefreshError(
                "PubMed redirected to a non-allow-listed URL"
            )
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def _default_fetch(url: str, *, timeout: float) -> bytes:
    parsed = urlsplit(url)
    if (
        parsed.scheme != "https"
        or parsed.hostname != "eutils.ncbi.nlm.nih.gov"
        or not parsed.path.startswith("/entrez/eutils/")
    ):
        raise PublicLiteratureRefreshError("refresh attempted a non-allow-listed URL")
    request = Request(
        url,
        headers={
            "Accept": "application/json, application/xml",
            "User-Agent": "aurora-agent/0.1",
        },
    )
    try:
        with build_opener(_AllowListedRedirectHandler()).open(
            request, timeout=timeout
        ) as response:  # nosec B310 - URL is allow-listed above
            body = response.read(MAX_RESPONSE_BYTES + 1)
    except OSError as error:
        raise PublicLiteratureRefreshError("PubMed request failed") from error
    if len(body) > MAX_RESPONSE_BYTES:
        raise PublicLiteratureRefreshError(
            "PubMed response exceeds the bounded byte limit"
        )
    return body


def _utc_timestamp(value: str | None) -> str:
    if value is None:
        return (
            datetime.now(timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z")
        )
    if not isinstance(value, str) or not _UTC_RE.fullmatch(value):
        raise PublicLiteratureRefreshError(
            "retrieved_at must be a UTC RFC3339 timestamp"
        )
    return value


def _pubmed_url(endpoint: str, **parameters: Any) -> str:
    if endpoint not in {"esearch.fcgi", "esummary.fcgi", "efetch.fcgi"}:
        raise PublicLiteratureRefreshError("unsupported PubMed endpoint")
    encoded = "&".join(
        f"{key}={quote(str(value), safe=',')}" for key, value in parameters.items()
    )
    return f"{PUBMED_EUTILS_BASE}{endpoint}?{encoded}"


def _ncbi_registration_parameters(
    ncbi_tool: str | None, ncbi_email: str | None
) -> dict[str, str]:
    """Validate an optional, already-registered NCBI application contact pair.

    NCBI treats these values as request identification, not as credentials.  This deliberately
    accepts neither an API key nor arbitrary query parameters.
    """

    if (ncbi_tool is None) != (ncbi_email is None):
        raise PublicLiteratureRefreshError(
            "ncbi_tool and ncbi_email must be provided together"
        )
    if ncbi_tool is None:
        return {}
    if type(ncbi_tool) is not str or _NCBI_TOOL_RE.fullmatch(ncbi_tool) is None:
        raise PublicLiteratureRefreshError(
            "ncbi_tool must be a bounded application name without spaces"
        )
    if (
        type(ncbi_email) is not str
        or not ncbi_email.isascii()
        or len(ncbi_email) > 254
        or _NCBI_EMAIL_RE.fullmatch(ncbi_email) is None
    ):
        raise PublicLiteratureRefreshError(
            "ncbi_email must be a bounded complete developer email address"
        )
    return {"tool": ncbi_tool, "email": ncbi_email}


def _normalise_date(value: Any) -> str | None:
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
    # Partial PubMed chronology must remain unknown.  Filling a missing day with
    # January 1 would fabricate a bound that the upstream record never supplied.
    match = re.match(r"^(\d{4})\s+([A-Za-z]{3})\s+(\d{1,2})$", text)
    if match:
        year, month, day = (
            int(match.group(1)),
            _MONTHS.get(match.group(2).title()),
            int(match.group(3)),
        )
        if month is None:
            return None
        try:
            return datetime(year, month, day).strftime("%Y-%m-%d")
        except ValueError:
            return None
    return None


def _bounded_abstract(value: str | None) -> tuple[str | None, bool]:
    if value is None:
        return None, False
    encoded = value.encode("utf-8")
    if len(encoded) <= MAX_ABSTRACT_BYTES:
        return value, False
    clipped = encoded[:MAX_ABSTRACT_BYTES].decode("utf-8", errors="ignore")
    return clipped, True


def _rust_record_projection(record: Mapping[str, Any]) -> dict[str, Any]:
    """Mirror serde's field order and skip-if-none rules for source/bundle digests."""

    projected: dict[str, Any] = {
        "source_id": record["source_id"],
        "specialty": record["specialty"],
        "pmid": record["pmid"],
        "title": record["title"],
        "journal": record["journal"],
        "publication_date": record.get("publication_date"),
    }
    if record.get("doi") is not None:
        projected["doi"] = record["doi"]
    if record.get("abstract_text") is not None:
        projected["abstract_text"] = record["abstract_text"]
    if record.get("abstract_truncated", False):
        projected["abstract_truncated"] = True
    if record.get("publication_types"):
        projected["publication_types"] = list(record["publication_types"])
    if record.get("mesh_terms"):
        projected["mesh_terms"] = list(record["mesh_terms"])
    return projected


def _rust_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), allow_nan=False
    ).encode("utf-8")


def _source_hash(bundle: Mapping[str, Any], source_id: str) -> str:
    records = [
        record for record in bundle["records"] if record["source_id"] == source_id
    ]
    records.sort(key=lambda record: (record["specialty"], record["pmid"]))
    return hashlib.sha256(
        _rust_json_bytes(
            {"records": [_rust_record_projection(record) for record in records]}
        )
    ).hexdigest()


def bundle_digest(bundle: Mapping[str, Any]) -> str:
    """Compute the Rust ``PublicLiteratureBundle::summary`` digest for a validated bundle."""

    projected = {
        "schema_version": bundle["schema_version"],
        "generated_at": bundle["generated_at"],
        "synthetic_data": bundle["synthetic_data"],
        "sources": [
            {
                "source_id": source["source_id"],
                "authority": source["authority"],
                "uri": source["uri"],
                "retrieved_at": source["retrieved_at"],
                "content_sha256": source["content_sha256"],
                "record_count": source["record_count"],
            }
            for source in bundle["sources"]
        ],
        "records": [_rust_record_projection(record) for record in bundle["records"]],
    }
    return hashlib.sha256(_rust_json_bytes(projected)).hexdigest()


def validate_public_literature_bundle(bundle: Mapping[str, Any]) -> None:
    """Apply the process-boundary checks needed before handing a candidate to Rust."""

    if (
        not isinstance(bundle, Mapping)
        or bundle.get("schema_version") != PUBLIC_LITERATURE_SCHEMA_VERSION
    ):
        raise PublicLiteratureRefreshError("unsupported public-literature schema")
    if bundle.get("synthetic_data") is not False:
        raise PublicLiteratureRefreshError(
            "public-literature snapshots require synthetic_data=false"
        )
    generated_at = bundle.get("generated_at")
    if not isinstance(generated_at, str) or not _UTC_RE.fullmatch(generated_at):
        raise PublicLiteratureRefreshError(
            "generated_at must be a UTC RFC3339 timestamp"
        )
    sources = bundle.get("sources")
    records = bundle.get("records")
    if not isinstance(sources, list) or not sources or len(sources) > MAX_SOURCE_COUNT:
        raise PublicLiteratureRefreshError(
            "public-literature sources are outside bounds"
        )
    if not isinstance(records, list) or not records or len(records) > MAX_RECORD_COUNT:
        raise PublicLiteratureRefreshError(
            "public-literature records are outside bounds"
        )
    source_by_id: dict[str, Mapping[str, Any]] = {}
    for source in sources:
        if not isinstance(source, Mapping):
            raise PublicLiteratureRefreshError(
                "public-literature source must be an object"
            )
        source_id = _text(source.get("source_id"), "source_id")
        assert source_id is not None
        if source_id in source_by_id:
            raise PublicLiteratureRefreshError("duplicate public-literature source_id")
        uri = _text(source.get("uri"), "source.uri")
        _text(source.get("authority"), "source.authority")
        retrieved_at = _text(source.get("retrieved_at"), "source.retrieved_at")
        assert uri is not None and retrieved_at is not None
        if (
            not uri.startswith(PUBMED_EUTILS_BASE)
            or not _UTC_RE.fullmatch(retrieved_at)
            or retrieved_at > generated_at
        ):
            raise PublicLiteratureRefreshError(
                "source is not an allow-listed UTC PubMed source"
            )
        digest = source.get("content_sha256")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise PublicLiteratureRefreshError("source content_sha256 is invalid")
        count = source.get("record_count")
        if (
            isinstance(count, bool)
            or not isinstance(count, int)
            or not 1 <= count <= MAX_RECORD_COUNT
        ):
            raise PublicLiteratureRefreshError("source record_count is invalid")
        source_by_id[source_id] = source

    seen_pmids: set[str] = set()
    counts: dict[str, int] = {source_id: 0 for source_id in source_by_id}
    for record in records:
        if not isinstance(record, Mapping):
            raise PublicLiteratureRefreshError(
                "public-literature record must be an object"
            )
        source_id = _text(record.get("source_id"), "record.source_id")
        specialty = _text(record.get("specialty"), "record.specialty")
        pmid = _text(record.get("pmid"), "record.pmid")
        title = _text(record.get("title"), "record.title")
        journal = _text(record.get("journal"), "record.journal")
        assert (
            source_id is not None
            and specialty is not None
            and pmid is not None
            and title is not None
            and journal is not None
        )
        if source_id not in source_by_id or specialty not in PUBMED_SPECIALTY_LANES:
            raise PublicLiteratureRefreshError("record source or specialty is invalid")
        if not pmid.isdigit() or len(pmid) > 32 or pmid in seen_pmids:
            raise PublicLiteratureRefreshError("record PMID is invalid or duplicated")
        seen_pmids.add(pmid)
        counts[source_id] += 1
        publication_date = record.get("publication_date")
        if publication_date is not None and (
            not isinstance(publication_date, str)
            or _normalise_date(publication_date) != publication_date
        ):
            raise PublicLiteratureRefreshError("record publication_date is invalid")
        doi = record.get("doi")
        if doi is not None and (
            not isinstance(doi, str)
            or not doi.startswith("10.")
            or len(doi.encode("utf-8")) > 512
        ):
            raise PublicLiteratureRefreshError("record DOI is invalid")
        abstract = record.get("abstract_text")
        if abstract is not None:
            if not isinstance(abstract, str):
                raise PublicLiteratureRefreshError(
                    "record abstract_text must be a string"
                )
            _text(abstract, "record.abstract_text")
            if len(abstract.encode("utf-8")) > MAX_ABSTRACT_BYTES:
                raise PublicLiteratureRefreshError(
                    "record abstract exceeds its safety bound"
                )
        abstract_truncated = record.get("abstract_truncated", False)
        if not isinstance(abstract_truncated, bool):
            raise PublicLiteratureRefreshError(
                "record abstract_truncated must be boolean"
            )
        if abstract_truncated and abstract is None:
            raise PublicLiteratureRefreshError(
                "missing abstract cannot be marked truncated"
            )
        for key in ("publication_types", "mesh_terms"):
            values = record.get(key, [])
            if not isinstance(values, list) or len(values) > MAX_TAGS:
                raise PublicLiteratureRefreshError(f"record {key} is outside bounds")
            for value in values:
                if not isinstance(value, str):
                    raise PublicLiteratureRefreshError(
                        f"record {key} values must be strings"
                    )
                _text(value, f"record.{key}")
    for source_id, source in source_by_id.items():
        if (
            counts[source_id] != source["record_count"]
            or hashlib.sha256(
                _rust_json_bytes(
                    {
                        "records": [
                            _rust_record_projection(record)
                            for record in sorted(
                                (r for r in records if r["source_id"] == source_id),
                                key=lambda r: (r["specialty"], r["pmid"]),
                            )
                        ]
                    }
                )
            ).hexdigest()
            != source["content_sha256"]
        ):
            raise PublicLiteratureRefreshError(
                f"source {source_id} content hash or count is invalid"
            )


def _summary(
    bundle: Mapping[str, Any], *, output_path: str | None = None, network: bool = True
) -> PublicLiteratureRefreshReport:
    counts: dict[str, int] = {}
    for record in bundle["records"]:
        specialty = str(record["specialty"])
        counts[specialty] = counts.get(specialty, 0) + 1
    return PublicLiteratureRefreshReport(
        schema_version=PUBLIC_LITERATURE_SCHEMA_VERSION,
        bundle_digest=bundle_digest(bundle),
        generated_at=str(bundle["generated_at"]),
        source_count=len(bundle["sources"]),
        record_count=len(bundle["records"]),
        abstract_count=sum(
            1 for record in bundle["records"] if record.get("abstract_text") is not None
        ),
        specialty_counts=counts,
        output_path=output_path,
        network=network,
    )


def refresh_neurosurgical_public_literature(
    *,
    fetch: PubMedFetcher | None = None,
    per_specialty_limit: int = 10,
    specialty_lanes: Sequence[str] | None = None,
    retrieved_at: str | None = None,
    timeout: float = 30.0,
    ncbi_tool: str | None = None,
    ncbi_email: str | None = None,
) -> tuple[dict[str, Any], PublicLiteratureRefreshReport]:
    """Fetch a bounded PubMed lane set and return a validated real-data snapshot plus report.

    Omitting ``specialty_lanes`` preserves the original six-lane refresh.  A caller that supplies
    lanes can narrow dispatch to 1..6 names from the fixed :data:`PUBMED_SPECIALTY_LANES`
    catalogue; arbitrary query text is never accepted through this surface.  Deployments may pass
    an NCBI-registered ``ncbi_tool`` and developer ``ncbi_email`` pair.  Both values are added to
    every E-utility request but are omitted from the returned bundle and report.  Supplying them
    does not itself register them with NCBI.
    """

    if (
        isinstance(per_specialty_limit, bool)
        or not 1 <= per_specialty_limit <= MAX_PER_SPECIALTY_LIMIT
    ):
        raise PublicLiteratureRefreshError(
            f"per_specialty_limit must be between 1 and {MAX_PER_SPECIALTY_LIMIT}"
        )
    if specialty_lanes is None:
        selected_lanes = tuple(PUBMED_SPECIALTY_LANES)
    else:
        if isinstance(specialty_lanes, (str, bytes, bytearray)) or not isinstance(
            specialty_lanes, Sequence
        ):
            raise PublicLiteratureRefreshError(
                "specialty_lanes must be a sequence of allow-listed lane names"
            )
        requested_lanes = tuple(specialty_lanes)
        if not 1 <= len(requested_lanes) <= MAX_PUBMED_LANES:
            raise PublicLiteratureRefreshError(
                f"specialty_lanes must contain 1..{MAX_PUBMED_LANES} lanes"
            )
        if any(
            not isinstance(lane, str) or lane not in PUBMED_SPECIALTY_LANES
            for lane in requested_lanes
        ):
            raise PublicLiteratureRefreshError(
                "specialty_lanes contains a non-allow-listed lane"
            )
        if len(set(requested_lanes)) != len(requested_lanes):
            raise PublicLiteratureRefreshError(
                "specialty_lanes contains a duplicate lane"
            )
        selected = set(requested_lanes)
        selected_lanes = tuple(
            lane for lane in PUBMED_SPECIALTY_LANES if lane in selected
        )
    if isinstance(timeout, bool) or not 1 <= timeout <= 120:
        raise PublicLiteratureRefreshError("timeout must be between 1 and 120 seconds")
    registration_parameters = _ncbi_registration_parameters(ncbi_tool, ncbi_email)
    generated_at = _utc_timestamp(retrieved_at)
    network = fetch is None
    if fetch is None:
        last_request = 0.0

        def fetch(url: str) -> bytes:
            nonlocal last_request
            elapsed = time.monotonic() - last_request
            if elapsed < 0.34:
                time.sleep(0.34 - elapsed)
            body = _default_fetch(url, timeout=timeout)
            last_request = time.monotonic()
            return body

    def fetch_once(url: str) -> bytes | str | Mapping[str, Any]:
        try:
            return fetch(url)
        except PublicLiteratureRefreshError:
            raise
        except Exception as error:
            raise PublicLiteratureRefreshError("PubMed transport failed") from error

    records: list[dict[str, Any]] = []
    sources: list[dict[str, Any]] = []
    seen_pmids: set[str] = set()
    for specialty in selected_lanes:
        term = PUBMED_SPECIALTY_LANES[specialty]
        search_uri = _pubmed_url(
            "esearch.fcgi",
            db="pubmed",
            term=term,
            retmax=per_specialty_limit,
            retmode="json",
            sort="pub_date",
        )
        search_request_uri = _pubmed_url(
            "esearch.fcgi",
            db="pubmed",
            term=term,
            retmax=per_specialty_limit,
            retmode="json",
            sort="pub_date",
            **registration_parameters,
        )
        search = _json_response(fetch_once(search_request_uri))
        result = search.get("esearchresult")
        ids = result.get("idlist") if isinstance(result, Mapping) else None
        if not isinstance(ids, list) or not ids:
            raise PublicLiteratureRefreshError(
                f"PubMed returned no records for specialty lane {specialty}"
            )
        ids = [
            str(value) for value in ids[:per_specialty_limit] if str(value).isdigit()
        ]
        if not ids:
            raise PublicLiteratureRefreshError(
                f"PubMed returned no valid PMID records for specialty lane {specialty}"
            )
        joined = ",".join(ids)
        summary_uri = _pubmed_url(
            "esummary.fcgi",
            db="pubmed",
            id=joined,
            retmode="json",
            **registration_parameters,
        )
        summary_payload = _json_response(fetch_once(summary_uri))
        summary_result = summary_payload.get("result")
        if not isinstance(summary_result, Mapping):
            raise PublicLiteratureRefreshError(
                "PubMed summary response has no result object"
            )
        fetch_uri = _pubmed_url(
            "efetch.fcgi",
            db="pubmed",
            id=joined,
            rettype="abstract",
            retmode="xml",
            **registration_parameters,
        )
        xml_root = _xml_response(fetch_once(fetch_uri))
        xml_content: dict[str, dict[str, Any]] = {}
        for article in xml_root.findall(".//PubmedArticle"):
            pmid_node = article.find("./MedlineCitation/PMID")
            if pmid_node is None or not (pmid_node.text or "").strip():
                continue
            pmid = (pmid_node.text or "").strip()
            abstract_parts: list[str] = []
            for node in article.findall(
                "./MedlineCitation/Article/Abstract/AbstractText"
            ):
                part = " ".join("".join(node.itertext()).split())
                if not part:
                    continue
                label = (node.attrib.get("Label") or "").strip()
                abstract_parts.append(f"{label}: {part}" if label else part)
            abstract, truncated = _bounded_abstract(
                " ".join(abstract_parts) if abstract_parts else None
            )
            publication_types = [
                " ".join("".join(node.itertext()).split())
                for node in article.findall(
                    "./MedlineCitation/Article/PublicationTypeList/PublicationType"
                )
            ]
            mesh_terms = [
                " ".join("".join(node.itertext()).split())
                for node in article.findall(
                    "./MedlineCitation/MeshHeadingList/MeshHeading/DescriptorName"
                )
            ]
            xml_content[pmid] = {
                "abstract_text": abstract,
                "abstract_truncated": truncated,
                "publication_types": [value for value in publication_types if value],
                "mesh_terms": [value for value in mesh_terms if value],
            }
        lane_records: list[dict[str, Any]] = []
        for pmid in ids:
            if pmid in seen_pmids:
                continue
            article = summary_result.get(pmid)
            if not isinstance(article, Mapping):
                continue
            title = _text(article.get("title"), f"PubMed {pmid} title")
            journal = _text(
                article.get("fulljournalname") or article.get("source"),
                f"PubMed {pmid} journal",
            )
            assert title is not None and journal is not None
            article_ids = article.get("articleids", [])
            doi = None
            if isinstance(article_ids, list):
                for identifier in article_ids:
                    if (
                        isinstance(identifier, Mapping)
                        and identifier.get("idtype") == "doi"
                    ):
                        doi = _text(
                            identifier.get("value"),
                            f"PubMed {pmid} doi",
                            required=False,
                        )
                        break
            content = xml_content.get(pmid, {})
            record = {
                "source_id": f"pubmed_{specialty}",
                "specialty": specialty,
                "pmid": pmid,
                "title": title,
                "journal": journal,
                "publication_date": _normalise_date(
                    article.get("epubdate") or article.get("pubdate")
                ),
                "doi": doi,
                "abstract_text": content.get("abstract_text"),
                "abstract_truncated": bool(content.get("abstract_truncated", False)),
                "publication_types": list(content.get("publication_types", [])),
                "mesh_terms": list(content.get("mesh_terms", [])),
            }
            lane_records.append(record)
            records.append(record)
            seen_pmids.add(pmid)
        if not lane_records:
            raise PublicLiteratureRefreshError(
                f"PubMed lane {specialty} produced no unique citation records"
            )
        sources.append(
            {
                "source_id": f"pubmed_{specialty}",
                "authority": PUBMED_AUTHORITY,
                "uri": search_uri,
                "retrieved_at": generated_at,
                "content_sha256": "0" * 64,
                "record_count": len(lane_records),
            }
        )
    if len(records) > MAX_TOTAL_RECORDS:
        raise PublicLiteratureRefreshError(
            "refresh produced more records than the bounded total"
        )
    bundle: dict[str, Any] = {
        "schema_version": PUBLIC_LITERATURE_SCHEMA_VERSION,
        "generated_at": generated_at,
        "synthetic_data": False,
        "sources": sources,
        "records": records,
    }
    for source in sources:
        source["content_sha256"] = _source_hash(bundle, source["source_id"])
    validate_public_literature_bundle(bundle)
    return bundle, _summary(bundle, network=network)


def atomic_refresh_neurosurgical_public_literature(
    output_path: str | os.PathLike[str],
    *,
    fetch: PubMedFetcher | None = None,
    per_specialty_limit: int = 10,
    specialty_lanes: Sequence[str] | None = None,
    retrieved_at: str | None = None,
    timeout: float = 30.0,
    ncbi_tool: str | None = None,
    ncbi_email: str | None = None,
) -> PublicLiteratureRefreshReport:
    """Refresh to a same-directory candidate and replace ``output_path`` only after validation."""

    destination = Path(output_path)
    if destination.exists() and not destination.is_file():
        raise PublicLiteratureRefreshError("refresh output path is not a file")
    destination.parent.mkdir(parents=True, exist_ok=True)
    bundle, report = refresh_neurosurgical_public_literature(
        fetch=fetch,
        per_specialty_limit=per_specialty_limit,
        specialty_lanes=specialty_lanes,
        retrieved_at=retrieved_at,
        timeout=timeout,
        ncbi_tool=ncbi_tool,
        ncbi_email=ncbi_email,
    )
    candidate = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            prefix=f".{destination.name}.",
            suffix=".candidate",
            dir=destination.parent,
            delete=False,
        ) as handle:
            candidate = Path(handle.name)
            json.dump(
                bundle,
                handle,
                ensure_ascii=False,
                sort_keys=False,
                separators=(",", ":"),
            )
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(candidate, destination)
    finally:
        if candidate is not None and candidate.exists():
            candidate.unlink(missing_ok=True)
    return PublicLiteratureRefreshReport(
        **{**report.__dict__, "output_path": str(destination)}
    )


__all__ = [
    "PUBLIC_LITERATURE_SCHEMA_VERSION",
    "PUBMED_AUTHORITY",
    "PUBMED_EUTILS_BASE",
    "PUBMED_RECORD_BASE",
    "PUBMED_SPECIALTY_LANES",
    "MAX_SOURCE_COUNT",
    "MAX_RECORD_COUNT",
    "MAX_PER_SPECIALTY_LIMIT",
    "PubMedFetcher",
    "PublicLiteratureRefreshError",
    "PublicLiteratureRefreshReport",
    "bundle_digest",
    "validate_public_literature_bundle",
    "refresh_neurosurgical_public_literature",
    "atomic_refresh_neurosurgical_public_literature",
]
