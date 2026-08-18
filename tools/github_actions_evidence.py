#!/usr/bin/env python3
"""Export bounded GitHub Actions provider and evidence handoff documents.

Manual mode reads caller-selected rows from local JSON files. Discovery mode uses a caller-supplied
GitHub token to retrieve one run, its bounded jobs, and—when requested—bounded artifact metadata.
Remote artifact/log bytes are downloaded only when the explicit byte-collection switch is enabled;
that path is HTTPS-only, size-bounded, and hashes response bytes locally without extracting archives
or interpreting logs. Attestations are accepted only through an explicit bounded caller file.

The exporter is an ingestion helper, not an evidence verifier. It emits a provider payload, an
optional collection envelope, and (when an explicit CI plan is supplied) an exact
``CiProviderEvidenceRequest`` handoff for the Rust audit/registry. It never authenticates a
provider, verifies a signature, executes checks, or turns a locator/digest declaration into proof
of remote bytes.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any, Mapping, Sequence
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen


MAX_INPUT_BYTES = 2 * 1024 * 1024
MAX_CHECKS = 64
MAX_TEXT_BYTES = 512
MAX_DURATION_MS = 7 * 24 * 60 * 60 * 1000
MAX_API_RESPONSE_BYTES = 2 * 1024 * 1024
MAX_REMOTE_BYTES = 16 * 1024 * 1024
MAX_REMOTE_TOTAL_BYTES = 256 * 1024 * 1024
MAX_DISCOVERY_PAGE_SIZE = MAX_CHECKS + 1
MAX_EVIDENCE_ROWS = 128
MAX_EVIDENCE_PAGE_SIZE = MAX_EVIDENCE_ROWS + 1
MAX_DISCOVERY_TIMEOUT_SECONDS = 30
SCHEMA = "bioprism-actions-github-provider-payload/0.1"
COLLECTION_SCHEMA = "bioprism-actions-github-provider-evidence-collection/0.1"
PROVIDER = "github_actions"
DIGEST_SCOPE_PROVIDER_METADATA = "provider_metadata"
DIGEST_SCOPE_CALLER_DECLARED = "caller_declared"
DIGEST_SCOPE_LOCAL_RESPONSE_BYTES = "local_response_bytes"


class ExportError(ValueError):
    """A caller-controlled input cannot be represented safely in the payload."""


def _optional_path(value: str) -> Path | None:
    return Path(value) if value else None


def _validated_path(field: str, value: Path) -> Path:
    text = str(value)
    if not text.strip() or len(text.encode("utf-8")) > MAX_TEXT_BYTES:
        raise ExportError(f"{field} must be non-empty and at most {MAX_TEXT_BYTES} UTF-8 bytes")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in text):
        raise ExportError(f"{field} must not contain control characters")
    return value


def _text(field: str, value: Any, *, required: bool = True) -> str | None:
    if value is None:
        if required:
            raise ExportError(f"{field} must be a non-empty string")
        return None
    if not isinstance(value, (str, int)) or isinstance(value, bool):
        raise ExportError(f"{field} must be a string or integer")
    result = str(value)
    if not result.strip() or len(result.encode("utf-8")) > MAX_TEXT_BYTES:
        raise ExportError(f"{field} must be non-empty and at most {MAX_TEXT_BYTES} UTF-8 bytes")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in result):
        raise ExportError(f"{field} must not contain control characters")
    return result


def _mapping(field: str, value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ExportError(f"{field} must be an object")
    return value


def _read_json(path: Path, field: str) -> Any:
    try:
        size = path.stat().st_size
    except OSError as error:
        raise ExportError(f"cannot stat {field} {path}: {error}") from error
    if size > MAX_INPUT_BYTES:
        raise ExportError(f"{field} exceeds the {MAX_INPUT_BYTES}-byte input bound")
    try:
        raw = path.read_text(encoding="utf-8")
        return json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ExportError(f"cannot read {field} {path}: {error}") from error


def _api_url(value: Any) -> str:
    text = _text("api-url", value)
    assert text is not None
    parsed = urlparse(text)
    if parsed.scheme != "https" or not parsed.netloc or parsed.params or parsed.query or parsed.fragment:
        raise ExportError("api-url must be an absolute HTTPS URL without query or fragment")
    return text.rstrip("/")


def _repository(value: Any) -> str:
    text = _text("repository", value)
    assert text is not None
    parts = text.split("/")
    if len(parts) != 2 or any(not part or part in {".", ".."} for part in parts):
        raise ExportError("repository must have the owner/name form")
    return text


def _run_id(value: Any) -> str:
    text = _text("run.id", value)
    assert text is not None
    if "/" in text or "\\" in text:
        raise ExportError("run.id must be a single path segment")
    return text


def _api_json(url: str, token: str, field: str) -> Mapping[str, Any]:
    request = Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "aurora-agent-github-actions-evidence",
        },
        method="GET",
    )
    request.add_unredirected_header("Authorization", f"Bearer {token}")
    try:
        with urlopen(request, timeout=MAX_DISCOVERY_TIMEOUT_SECONDS) as response:
            content_length = response.headers.get("Content-Length")
            if content_length is not None:
                try:
                    if int(content_length) > MAX_API_RESPONSE_BYTES:
                        raise ExportError(f"{field} response exceeds the {MAX_API_RESPONSE_BYTES}-byte bound")
                except ValueError as error:
                    raise ExportError(f"{field} response has an invalid Content-Length") from error
            raw = response.read(MAX_API_RESPONSE_BYTES + 1)
    except HTTPError as error:
        raise ExportError(f"GitHub API {field} request failed with HTTP {error.code}") from error
    except (URLError, OSError, TimeoutError) as error:
        raise ExportError(f"GitHub API {field} request failed: {error}") from error
    if len(raw) > MAX_API_RESPONSE_BYTES:
        raise ExportError(f"{field} response exceeds the {MAX_API_RESPONSE_BYTES}-byte bound")
    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ExportError(f"GitHub API {field} response is not valid UTF-8 JSON") from error
    return _mapping(f"GitHub API {field}", value)


class _HttpsOnlyRedirectHandler(HTTPRedirectHandler):
    """Reject insecure redirect targets and prevent credentials crossing redirects."""

    def __init__(self, field: str) -> None:
        super().__init__()
        self.field = field

    def redirect_request(self, request: Request, *args: Any, **kwargs: Any) -> Request | None:
        redirected = super().redirect_request(request, *args, **kwargs)
        if redirected is None:
            return None
        parsed = urlparse(redirected.full_url)
        if parsed.scheme != "https" or not parsed.netloc or parsed.fragment:
            raise ExportError(f"{self.field} redirect target must be an absolute HTTPS URL without fragment")
        # Request.add_unredirected_header is intentional on the first request, but be explicit
        # here as well: a signed artifact URL may redirect to another HTTPS host and must not
        # receive the caller's GitHub token.
        redirected.headers.pop("Authorization", None)
        redirected.unredirected_hdrs.pop("Authorization", None)
        return redirected


def _open_remote(request: Request, field: str) -> Any:
    opener = build_opener(_HttpsOnlyRedirectHandler(field))
    return opener.open(request, timeout=MAX_DISCOVERY_TIMEOUT_SECONDS)


def _remote_bytes(url: Any, token: Any, field: str, *, limit: int = MAX_REMOTE_BYTES) -> bytes:
    """Fetch one HTTPS response under hard per-response and credential-forwarding bounds."""

    if limit <= 0 or limit > MAX_REMOTE_BYTES:
        raise ExportError(f"{field} requested an invalid remote byte limit")
    locator = _text(field, url)
    assert locator is not None
    parsed = urlparse(locator)
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.fragment
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ExportError(f"{field} must be an absolute HTTPS URL without credentials or fragment")
    token_text = None
    if token not in (None, ""):
        token_text = _text("github-token", token)
        assert token_text is not None
        if any(ord(character) < 0x20 or ord(character) == 0x7F for character in token_text):
            raise ExportError("github-token must not contain control characters")
    request = Request(
        locator,
        headers={
            "Accept": "application/octet-stream",
            "User-Agent": "aurora-agent-github-actions-evidence",
        },
        method="GET",
    )
    if token_text is not None:
        # Unlike a normal header, urllib does not copy this across redirects. The redirect
        # handler also strips it defensively from any redirected Request object.
        request.add_unredirected_header("Authorization", f"Bearer {token_text}")
    try:
        with _open_remote(request, field) as response:
            content_length = response.headers.get("Content-Length")
            if content_length is not None:
                try:
                    parsed_length = int(content_length)
                except (TypeError, ValueError) as error:
                    raise ExportError(f"{field} response has an invalid Content-Length") from error
                if parsed_length < 0 or parsed_length > limit:
                    raise ExportError(f"{field} response exceeds the {limit}-byte bound")
            raw = response.read(limit + 1)
    except HTTPError as error:
        raise ExportError(f"remote {field} request failed with HTTP {error.code}") from error
    except (URLError, OSError, TimeoutError) as error:
        raise ExportError(f"remote {field} request failed: {error}") from error
    if len(raw) > limit:
        raise ExportError(f"{field} response exceeds the {limit}-byte bound")
    return raw


def _reported_total_count(document: Mapping[str, Any], field: str, maximum: int) -> None:
    """Reject a bounded first page when the provider reports more rows behind it."""

    total = document.get("total_count")
    if total is None:
        return
    if isinstance(total, bool) or not isinstance(total, int) or total < 0:
        raise ExportError(f"GitHub API {field}.total_count must be a non-negative integer")
    if total > maximum:
        raise ExportError(
            f"GitHub API returned more than {maximum} {field}; refusing a partial payload"
        )


def _timestamp(field: str, value: Any) -> datetime:
    text = _text(field, value)
    assert text is not None
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError as error:
        raise ExportError(f"{field} must be an RFC 3339 timestamp") from error
    if parsed.tzinfo is None:
        raise ExportError(f"{field} must include a timezone")
    return parsed.astimezone(timezone.utc)


def _job_duration(job: Mapping[str, Any], index: int) -> int | None:
    started = job.get("started_at")
    completed = job.get("completed_at")
    if started is None or completed is None:
        return None
    delta = _timestamp(f"jobs[{index}].started_at", started)
    end = _timestamp(f"jobs[{index}].completed_at", completed)
    duration = end - delta
    duration_ms = duration.days * 24 * 60 * 60 * 1000 + duration.seconds * 1000 + duration.microseconds // 1000
    if duration_ms < 0 or duration_ms > MAX_DURATION_MS:
        raise ExportError(f"jobs[{index}].duration_ms must be between 0 and {MAX_DURATION_MS}")
    return duration_ms


def _sha256_mapping(value: Mapping[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def _sha256_digest(field: str, value: Any) -> str:
    digest = _text(field, value)
    assert digest is not None
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise ExportError(f"{field} must be a lowercase SHA-256 digest")
    return digest


def _rows_array(field: str, value: Any, *, required: bool = False) -> list[Mapping[str, Any]]:
    if value is None:
        if required:
            raise ExportError(f"{field} must be an array")
        return []
    if isinstance(value, Mapping):
        value = value.get(field)
    if not isinstance(value, list):
        raise ExportError(f"{field} must be an array or an object containing {field}")
    if len(value) > MAX_EVIDENCE_ROWS:
        raise ExportError(f"{field} cannot contain more than {MAX_EVIDENCE_ROWS} rows")
    return [_mapping(f"{field}[{index}]", row) for index, row in enumerate(value)]


def _artifact_rows(value: Any, run_id: Any, *, field: str = "artifacts") -> list[dict[str, Any]]:
    """Normalize GitHub artifact metadata into the Rust artifact-row shape.

    GitHub's artifact-list response does not provide a cryptographic archive digest. When no
    caller digest is supplied, the emitted digest is over the selected metadata and is deliberately
    described as a metadata digest by the collection envelope.
    """

    resolved_run_id = _run_id(run_id)
    rows = _rows_array(field, value)
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(rows):
        artifact_id = _text(f"{field}[{index}].id", raw.get("id"))
        assert artifact_id is not None
        if artifact_id in seen:
            raise ExportError(f"duplicate {field} id {artifact_id!r}")
        seen.add(artifact_id)
        name = _text(
            f"{field}[{index}].name",
            raw.get("name", raw.get("kind", artifact_id)),
        )
        assert name is not None
        uri = _text(
            f"{field}[{index}].uri",
            raw.get("uri", raw.get("archive_download_url", raw.get("url"))),
            required=False,
        )
        supplied_digest = raw.get("digest")
        if supplied_digest is not None:
            digest = _sha256_digest(f"{field}[{index}].digest", supplied_digest)
        else:
            metadata: dict[str, Any] = {"id": artifact_id, "name": name}
            for key in ("size_in_bytes", "expired", "created_at", "expires_at"):
                if key in raw:
                    candidate = raw[key]
                    if key == "size_in_bytes":
                        if isinstance(candidate, bool) or not isinstance(candidate, int) or candidate < 0:
                            raise ExportError(f"{field}[{index}].size_in_bytes must be a non-negative integer")
                    elif key == "expired" and not isinstance(candidate, bool):
                        raise ExportError(f"{field}[{index}].expired must be a boolean")
                    elif key in {"created_at", "expires_at"}:
                        _timestamp(f"{field}[{index}].{key}", candidate)
                    metadata[key] = candidate
            if uri is not None:
                metadata["uri"] = uri
            digest = _sha256_mapping(metadata)
        normalized.append(
            {
                "id": artifact_id,
                "kind": name,
                "digest": digest,
                "run_id": resolved_run_id,
                "provider": PROVIDER,
                "digest_scope": (
                    DIGEST_SCOPE_CALLER_DECLARED
                    if supplied_digest is not None
                    else DIGEST_SCOPE_PROVIDER_METADATA
                ),
                **({"uri": uri} if uri is not None else {}),
            }
        )
    return normalized


def _log_rows(value: Any, run_id: Any, *, field: str = "logs") -> list[dict[str, Any]]:
    """Validate caller-supplied log rows without dereferencing their locators."""

    resolved_run_id = _run_id(run_id)
    rows = _rows_array(field, value)
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(rows):
        log_id = _text(f"{field}[{index}].id", raw.get("id"))
        assert log_id is not None
        if log_id in seen:
            raise ExportError(f"duplicate {field} id {log_id!r}")
        seen.add(log_id)
        digest = _sha256_digest(f"{field}[{index}].digest", raw.get("digest"))
        check = _text(f"{field}[{index}].check", raw.get("check"), required=False)
        uri = _text(f"{field}[{index}].uri", raw.get("uri"), required=False)
        truncated = raw.get("truncated", False)
        if not isinstance(truncated, bool):
            raise ExportError(f"{field}[{index}].truncated must be a boolean")
        normalized.append(
            {
                "id": log_id,
                "digest": digest,
                "run_id": resolved_run_id,
                "provider": PROVIDER,
                "truncated": truncated,
                "digest_scope": DIGEST_SCOPE_CALLER_DECLARED,
                **({"check": check} if check is not None else {}),
                **({"uri": uri} if uri is not None else {}),
            }
        )
    return normalized


def _attestation_rows(value: Any, *, field: str = "attestations") -> list[dict[str, Any]]:
    rows = _rows_array(field, value)
    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(rows):
        attestation_id = _text(f"{field}[{index}].id", raw.get("id"))
        assert attestation_id is not None
        if attestation_id in seen:
            raise ExportError(f"duplicate {field} id {attestation_id!r}")
        seen.add(attestation_id)
        subject_digest = raw.get("subject_digest")
        normalized.append(
            {
                "id": attestation_id,
                "subject": _text(f"{field}[{index}].subject", raw.get("subject")),
                "issuer": _text(f"{field}[{index}].issuer", raw.get("issuer")),
                "statement_digest": _sha256_digest(
                    f"{field}[{index}].statement_digest", raw.get("statement_digest")
                ),
                "method": _text(f"{field}[{index}].method", raw.get("method")),
                **(
                    {
                        "subject_digest": _sha256_digest(
                            f"{field}[{index}].subject_digest", subject_digest
                        )
                    }
                    if subject_digest is not None
                    else {}
                ),
            }
        )
    return normalized


def _job_log_rows(raw_jobs: Sequence[Mapping[str, Any]], run_id: Any) -> tuple[list[dict[str, Any]], int]:
    """Create bounded log locator rows from the already retrieved job metadata."""

    resolved_run_id = _run_id(run_id)
    rows: list[dict[str, Any]] = []
    missing = 0
    seen: set[str] = set()
    for index, job in enumerate(raw_jobs):
        job_id = _text(f"jobs[{index}].id", job.get("id", job.get("name")))
        assert job_id is not None
        name = _text(f"jobs[{index}].name", job.get("name", job_id))
        assert name is not None
        uri = _text(f"jobs[{index}].logs_url", job.get("logs_url"), required=False)
        if uri is None:
            missing += 1
            continue
        log_id = _text(f"jobs[{index}].log_id", f"job-{job_id}-logs")
        assert log_id is not None
        if log_id in seen:
            raise ExportError(f"duplicate discovered log id {log_id!r}")
        seen.add(log_id)
        metadata = {
            "check": name,
            "job_id": job_id,
            "locator": uri,
            "scope": "job_log_locator_metadata",
        }
        rows.append(
            {
                "id": log_id,
                "digest": _sha256_mapping(metadata),
                "check": name,
                "run_id": resolved_run_id,
                "provider": PROVIDER,
                "uri": uri,
                "truncated": False,
                "digest_scope": DIGEST_SCOPE_PROVIDER_METADATA,
            }
        )
    if len(rows) > MAX_EVIDENCE_ROWS:
        raise ExportError(
            f"discovered logs contain more than {MAX_EVIDENCE_ROWS} locators; refusing a partial collection"
        )
    return rows, missing


def _download_evidence(
    artifacts: Sequence[Mapping[str, Any]],
    logs: Sequence[Mapping[str, Any]],
    token: Any,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    """Replace locator/metadata digests with SHA-256 digests of bounded response bytes."""

    total_bytes = 0
    downloaded_artifacts: list[dict[str, Any]] = []
    downloaded_logs: list[dict[str, Any]] = []

    def download_rows(
        source_rows: Sequence[Mapping[str, Any]],
        destination: list[dict[str, Any]],
        field: str,
    ) -> None:
        nonlocal total_bytes
        for index, row in enumerate(source_rows):
            uri = row.get("uri")
            if uri is None:
                raise ExportError(f"{field}[{index}].uri is required when download-evidence is enabled")
            remaining = MAX_REMOTE_TOTAL_BYTES - total_bytes
            if remaining <= 0:
                raise ExportError(f"remote evidence exceeds the {MAX_REMOTE_TOTAL_BYTES}-byte total bound")
            raw = _remote_bytes(uri, token, f"{field}[{index}].uri", limit=min(MAX_REMOTE_BYTES, remaining))
            total_bytes += len(raw)
            updated = dict(row)
            updated["digest"] = hashlib.sha256(raw).hexdigest()
            updated["digest_scope"] = DIGEST_SCOPE_LOCAL_RESPONSE_BYTES
            destination.append(updated)

    download_rows(artifacts, downloaded_artifacts, "artifacts")
    download_rows(logs, downloaded_logs, "logs")
    return (
        downloaded_artifacts,
        downloaded_logs,
        {
            "mode": "downloaded_and_sha256_hashed",
            "artifact_count": len(downloaded_artifacts),
            "log_count": len(downloaded_logs),
            "total_bytes": total_bytes,
            "max_bytes_per_response": MAX_REMOTE_BYTES,
            "max_total_bytes": MAX_REMOTE_TOTAL_BYTES,
        },
    )


def discover_github_actions_payload(
    *,
    token: Any,
    api_url: Any,
    repository: Any,
    run_id: Any,
    collect_evidence: bool = False,
    download_evidence: bool = False,
) -> dict[str, Any]:
    """Retrieve one bounded GitHub run and convert it to exporter input rows.

    The API response is treated as an observed provider payload, not as a signature or log proof.
    A response larger than the normalizer's check bound is refused instead of truncated, because a
    partial job list could make a required check look absent for the wrong reason.

    When ``collect_evidence`` is true, the collector also retrieves the bounded artifact metadata
    endpoint and derives log locator rows from the job response. When ``download_evidence`` is
    true, it implies collection and downloads each bounded HTTPS locator to compute a local byte
    digest. It does not extract archives, interpret logs, or verify signatures.
    """

    token_text = _text("github-token", token)
    assert token_text is not None
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in token_text):
        raise ExportError("github-token must not contain control characters")
    base = _api_url(api_url)
    repo = _repository(repository)
    selected_run_id = _run_id(run_id)
    encoded_repo = quote(repo, safe="/")
    encoded_run_id = quote(selected_run_id, safe="")
    run = _api_json(
        f"{base}/repos/{encoded_repo}/actions/runs/{encoded_run_id}",
        token_text,
        "run",
    )
    discovered_id = run.get("id", run.get("run_id"))
    if _run_id(discovered_id) != selected_run_id:
        raise ExportError("GitHub API run response id does not match the requested run id")
    jobs_document = _api_json(
        f"{base}/repos/{encoded_repo}/actions/runs/{encoded_run_id}/jobs?per_page={MAX_DISCOVERY_PAGE_SIZE}",
        token_text,
        "jobs",
    )
    raw_jobs = jobs_document.get("jobs")
    if not isinstance(raw_jobs, list):
        raise ExportError("GitHub API jobs response must contain a jobs array")
    _reported_total_count(jobs_document, "jobs", MAX_CHECKS)
    if not raw_jobs:
        raise ExportError("GitHub API jobs response contains no jobs")
    if len(raw_jobs) > MAX_CHECKS:
        raise ExportError(f"GitHub API returned more than {MAX_CHECKS} jobs; refusing a partial payload")
    rows: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for index, raw_job in enumerate(raw_jobs):
        job = _mapping(f"jobs[{index}]", raw_job)
        name = _text(f"jobs[{index}].name", job.get("name", job.get("id")))
        assert name is not None
        if name in seen_names:
            raise ExportError(f"duplicate check name {name!r}")
        seen_names.add(name)
        row: dict[str, Any] = {"name": name}
        status = _text(f"jobs[{index}].status", job.get("status"), required=False)
        conclusion = _text(f"jobs[{index}].conclusion", job.get("conclusion"), required=False)
        if conclusion is not None:
            row["conclusion"] = conclusion
        elif status is not None:
            row["status"] = status
        duration_ms = _job_duration(job, index)
        if duration_ms is not None:
            row["duration_ms"] = duration_ms
        detail = _text(f"jobs[{index}].html_url", job.get("html_url"), required=False)
        if detail is not None:
            row["detail"] = detail
        rows.append(row)
    artifact_rows: list[dict[str, Any]] = []
    log_rows: list[dict[str, Any]] = []
    missing_log_locator_count = 0
    download_stats: dict[str, Any] | None = None
    if collect_evidence or download_evidence:
        artifacts_document = _api_json(
            f"{base}/repos/{encoded_repo}/actions/runs/{encoded_run_id}/artifacts?per_page={MAX_EVIDENCE_PAGE_SIZE}",
            token_text,
            "artifacts",
        )
        _reported_total_count(artifacts_document, "artifacts", MAX_EVIDENCE_ROWS)
        artifact_rows = _artifact_rows(artifacts_document, selected_run_id)
        log_rows, missing_log_locator_count = _job_log_rows(raw_jobs, selected_run_id)
        if download_evidence:
            artifact_rows, log_rows, download_stats = _download_evidence(
                artifact_rows,
                log_rows,
                token_text,
            )
    return {
        "run": dict(run),
        "jobs": rows,
        "artifacts": artifact_rows,
        "logs": log_rows,
        "missing_log_locator_count": missing_log_locator_count,
        "download_stats": download_stats,
    }


def _run_from_event(event: Any) -> Mapping[str, Any]:
    if event is None:
        return {}
    root = _mapping("event", event)
    workflow_run = root.get("workflow_run")
    if workflow_run is not None:
        return _mapping("event.workflow_run", workflow_run)
    run = root.get("run")
    if run is not None:
        return _mapping("event.run", run)
    return root


def _check_rows(value: Any) -> Sequence[Mapping[str, Any]]:
    if isinstance(value, Mapping):
        value = value.get("jobs", value.get("checks"))
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ExportError("checks must be an array or an object containing jobs/checks")
    if not value:
        raise ExportError("checks must contain at least one job")
    if len(value) > MAX_CHECKS:
        raise ExportError(f"checks cannot contain more than {MAX_CHECKS} jobs")
    rows: list[Mapping[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(value):
        row = _mapping(f"checks[{index}]", raw)
        name = _text(
            f"checks[{index}].name",
            row.get("name", row.get("job_name", row.get("id"))),
        )
        assert name is not None
        if name in seen:
            raise ExportError(f"duplicate check name {name!r}")
        seen.add(name)
        status = _text(
            f"checks[{index}].conclusion",
            row.get("conclusion", row.get("status", row.get("result"))),
            required=False,
        ) or "unknown"
        duration = row.get("duration_ms")
        if duration is not None:
            if isinstance(duration, bool) or not isinstance(duration, int):
                raise ExportError(f"checks[{index}].duration_ms must be an integer")
            if duration < 0 or duration > MAX_DURATION_MS:
                raise ExportError(
                    f"checks[{index}].duration_ms must be between 0 and {MAX_DURATION_MS}"
                )
        detail = _text(f"checks[{index}].detail", row.get("detail"), required=False)
        normalized: dict[str, Any] = {"name": name, "conclusion": status}
        if duration is not None:
            normalized["duration_ms"] = duration
        if detail is not None:
            normalized["detail"] = detail
        supplied_digest = row.get("result_digest")
        if supplied_digest is not None:
            digest = _text(f"checks[{index}].result_digest", supplied_digest)
            assert digest is not None
            if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
                raise ExportError(f"checks[{index}].result_digest must be a lowercase SHA-256 digest")
            normalized["result_digest"] = digest
        rows.append(normalized)
    return rows


def build_payload(
    checks: Any,
    event: Any = None,
    *,
    run_id: Any = None,
    conclusion: Any = None,
    run_url: Any = None,
) -> dict[str, Any]:
    """Build the exact bounded provider payload consumed by Rust normalization."""

    run = _run_from_event(event)
    resolved_run_id = _text("run.id", run_id if run_id is not None else run.get("id", run.get("run_id")))
    resolved_conclusion = _text(
        "run.conclusion",
        conclusion if conclusion is not None else run.get("conclusion", run.get("result")),
        required=False,
    ) or "unknown"
    resolved_url = _text(
        "run.html_url",
        run_url
        if run_url is not None
        else run.get("html_url", run.get("run_url", run.get("url"))),
        required=False,
    )
    payload_run: dict[str, Any] = {"id": resolved_run_id, "conclusion": resolved_conclusion}
    if resolved_url is not None:
        payload_run["html_url"] = resolved_url
    return {"provider": "github_actions", "schema": SCHEMA, "run": payload_run, "jobs": list(_check_rows(checks))}


def canonical_bytes(value: Mapping[str, Any]) -> bytes:
    """Serialize exactly like the repository's content-addressing reference."""

    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
        "utf-8"
    )


def payload_digest(payload: Mapping[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def build_provider_evidence_request(
    ci: Any,
    payload: Mapping[str, Any],
    *,
    artifacts: Sequence[Mapping[str, Any]] = (),
    logs: Sequence[Mapping[str, Any]] = (),
    attestations: Sequence[Mapping[str, Any]] = (),
    source: str,
) -> dict[str, Any]:
    """Build the exact JSON envelope consumed by ``ci_provider_evidence_import``.

    The CI plan is intentionally caller-supplied. The exporter never infers required checks from
    observed jobs, because doing so would turn provider observations into release intent.
    """

    ci_mapping = _mapping("ci", ci)
    if source not in {"provider_observed", "caller_attested"}:
        raise ExportError("source must be provider_observed or caller_attested")
    return {
        "ci": dict(ci_mapping),
        "provider": PROVIDER,
        "payload": dict(_mapping("payload", payload)),
        "source": source,
        "artifacts": list(artifacts),
        "logs": list(logs),
        "attestations": list(attestations),
    }


def build_collection_envelope(
    payload: Mapping[str, Any],
    *,
    artifacts: Sequence[Mapping[str, Any]] = (),
    logs: Sequence[Mapping[str, Any]] = (),
    attestations: Sequence[Mapping[str, Any]] = (),
    discovery_mode: str,
    missing_log_locator_count: int = 0,
    attestation_mode: str = "not_requested",
    download_stats: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a transparent metadata collection envelope, not a Rust audit result."""

    if discovery_mode not in {"manual", "github_api"}:
        raise ExportError("discovery_mode must be manual or github_api")
    if attestation_mode not in {"not_requested", "caller_supplied"}:
        raise ExportError("attestation_mode must be not_requested or caller_supplied")
    if missing_log_locator_count < 0:
        raise ExportError("missing_log_locator_count cannot be negative")
    if download_stats is not None:
        stats = _mapping("download_stats", download_stats)
        for key in ("artifact_count", "log_count", "total_bytes", "max_bytes_per_response", "max_total_bytes"):
            value = stats.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise ExportError(f"download_stats.{key} must be a non-negative integer")
        if stats.get("mode") != "downloaded_and_sha256_hashed":
            raise ExportError("download_stats.mode must be downloaded_and_sha256_hashed")
    collection: dict[str, Any] = {
        "run_metadata": discovery_mode,
        "artifact_metadata": "github_api" if discovery_mode == "github_api" else "caller_supplied",
        "log_metadata": "job_log_locators" if discovery_mode == "github_api" else "caller_supplied",
        "attestations": attestation_mode,
        "execution": "not_started",
        "verification": "local_byte_hash_only" if download_stats is not None else "metadata_only",
        "limitations": (
            [
                "artifact and log SHA-256 digests cover the bounded response bytes retrieved locally",
                "artifact archives were not extracted or independently validated",
                "logs were not interpreted or executed",
                "attestation rows are declarations and are not signature-verified",
            ]
            if download_stats is not None
            else [
                "artifact digests are selected-metadata digests unless a caller supplied a digest",
                "log rows are locators and their digests cover locator metadata, not downloaded bytes",
                "attestation rows are declarations and are not signature-verified",
            ]
        ),
    }
    if download_stats is not None:
        collection["byte_collection"] = {
            "mode": "downloaded_and_sha256_hashed",
            "artifact_count": download_stats["artifact_count"],
            "log_count": download_stats["log_count"],
            "total_bytes": download_stats["total_bytes"],
            "max_bytes_per_response": download_stats["max_bytes_per_response"],
            "max_total_bytes": download_stats["max_total_bytes"],
        }
    if missing_log_locator_count:
        collection["missing_log_locator_count"] = missing_log_locator_count
    return {
        "schema": COLLECTION_SCHEMA,
        "provider": PROVIDER,
        "payload": dict(_mapping("payload", payload)),
        "artifacts": list(artifacts),
        "logs": list(logs),
        "attestations": list(attestations),
        "collection": collection,
    }


def write_document(document: Mapping[str, Any], output: Path, *, field: str = "output") -> str:
    """Write one deterministic JSON document and return its digest over bytes without the newline."""

    output = _validated_path(field, output)
    encoded = canonical_bytes(document)
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded + b"\n")
    except OSError as error:
        raise ExportError(f"cannot write {field} {output}: {error}") from error
    return hashlib.sha256(encoded).hexdigest()


def write_payload(
    payload: Mapping[str, Any],
    output: Path,
    *,
    github_output: Path | None = None,
    discovery_mode: str = "manual",
    collection: Mapping[str, Any] | None = None,
    collection_output: Path | None = None,
    evidence: Mapping[str, Any] | None = None,
    evidence_output: Path | None = None,
    download_stats: Mapping[str, Any] | None = None,
) -> str:
    """Write deterministic JSON and optional collection/evidence action outputs."""

    output = _validated_path("output path", output)
    if github_output is not None:
        github_output = _validated_path("GITHUB_OUTPUT path", github_output)
    if discovery_mode not in {"manual", "github_api"}:
        raise ExportError("discovery_mode must be manual or github_api")
    if collection is not None and collection_output is None:
        raise ExportError("collection-output is required when a collection is requested")
    if evidence is not None and evidence_output is None:
        raise ExportError("evidence-output is required when an evidence request is requested")
    digest = write_document(payload, output, field="output")
    collection_digest = (
        write_document(collection, collection_output, field="collection-output")
        if collection is not None and collection_output is not None
        else None
    )
    evidence_digest = (
        write_document(evidence, evidence_output, field="evidence-output")
        if evidence is not None and evidence_output is not None
        else None
    )
    if github_output is not None:
        try:
            with github_output.open("a", encoding="utf-8", newline="\n") as stream:
                stream.write(f"payload-path={output}\n")
                stream.write(f"payload-digest={digest}\n")
                stream.write(f"run-id={payload['run']['id']}\n")
                stream.write(f"check-count={len(payload['jobs'])}\n")
                stream.write(f"discovery-mode={discovery_mode}\n")
                if collection is not None and collection_output is not None and collection_digest is not None:
                    stream.write(f"collection-path={collection_output}\n")
                    stream.write(f"collection-digest={collection_digest}\n")
                if evidence is not None and evidence_output is not None and evidence_digest is not None:
                    stream.write(f"evidence-path={evidence_output}\n")
                    stream.write(f"evidence-digest={evidence_digest}\n")
                rows_document = evidence if evidence is not None else collection
                if rows_document is not None:
                    stream.write(f"artifact-count={len(rows_document.get('artifacts', []))}\n")
                    stream.write(f"log-count={len(rows_document.get('logs', []))}\n")
                    stream.write(f"attestation-count={len(rows_document.get('attestations', []))}\n")
                if download_stats is None:
                    stream.write("download-mode=disabled\n")
                    stream.write("downloaded-artifact-count=0\n")
                    stream.write("downloaded-log-count=0\n")
                    stream.write("downloaded-byte-count=0\n")
                else:
                    stream.write("download-mode=local_byte_hash_only\n")
                    stream.write(f"downloaded-artifact-count={download_stats['artifact_count']}\n")
                    stream.write(f"downloaded-log-count={download_stats['log_count']}\n")
                    stream.write(f"downloaded-byte-count={download_stats['total_bytes']}\n")
        except OSError as error:
            raise ExportError(f"cannot write GITHUB_OUTPUT {github_output}: {error}") from error
    return digest


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checks", type=Path, help="JSON array/object containing bounded job rows")
    parser.add_argument(
        "--discover",
        action="store_true",
        help="retrieve the selected run and jobs through the GitHub API instead of reading --checks",
    )
    parser.add_argument(
        "--collect-evidence",
        action="store_true",
        help="also collect bounded artifact metadata and job log locators during discovery",
    )
    parser.add_argument(
        "--download-evidence",
        action="store_true",
        help="explicitly download bounded HTTPS artifact/log responses and hash their bytes locally",
    )
    parser.add_argument("--github-token", default="", help="token for bounded GitHub API discovery")
    parser.add_argument("--api-url", default="", help="GitHub API base URL for discovery")
    parser.add_argument("--repository", default="", help="owner/name repository for discovery")
    parser.add_argument("--artifacts", type=Path, help="optional JSON artifact metadata/rows file for manual collection")
    parser.add_argument("--logs", type=Path, help="optional JSON log rows file for manual collection")
    parser.add_argument("--attestations", type=Path, help="optional JSON attestation rows file supplied by the caller")
    parser.add_argument("--ci", type=Path, help="CI plan JSON file required for an exact provider-evidence request")
    parser.add_argument(
        "--event",
        type=_optional_path,
        help="optional GitHub event JSON; defaults to GITHUB_EVENT_PATH",
    )
    parser.add_argument("--run-id", default="", help="optional run id; defaults to event/GITHUB_RUN_ID")
    parser.add_argument("--conclusion", default="", help="optional conclusion; defaults to event metadata")
    parser.add_argument("--run-url", default="", help="optional run URL; defaults to event metadata")
    parser.add_argument("--output", required=True, type=Path, help="deterministic provider payload output path")
    parser.add_argument(
        "--collection-output",
        type=Path,
        help="optional metadata collection envelope output path",
    )
    parser.add_argument(
        "--evidence-output",
        type=Path,
        help="optional exact CiProviderEvidenceRequest output path; requires --ci",
    )
    parser.add_argument("--github-output", type=Path, help="optional GITHUB_OUTPUT file")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.collect_evidence and not args.discover:
            raise ExportError("--collect-evidence requires --discover")
        if args.evidence_output is not None and args.ci is None:
            raise ExportError("--evidence-output requires --ci")
        if args.ci is not None and args.evidence_output is None:
            raise ExportError("--ci requires --evidence-output")
        if (
            (args.artifacts is not None or args.logs is not None or args.attestations is not None)
            and args.collection_output is None
            and args.evidence_output is None
        ):
            raise ExportError("artifact, log, and attestation inputs require collection-output or evidence-output")
        if args.discover and (args.artifacts is not None or args.logs is not None):
            raise ExportError("--artifacts and --logs cannot be combined with --discover")
        event_path = args.event or (
            Path(os.environ["GITHUB_EVENT_PATH"]) if os.environ.get("GITHUB_EVENT_PATH") else None
        )
        event = _read_json(event_path, "event") if event_path else None
        event_run = _run_from_event(event)
        event_run_id = event_run.get("id", event_run.get("run_id"))
        if args.run_id:
            resolved_run_id = args.run_id
        elif args.discover and isinstance(event, Mapping) and "workflow_run" in event:
            # A workflow_run consumer runs under a new workflow id but normally wants to inspect
            # the completed upstream run named by the event payload.
            resolved_run_id = event_run_id or os.environ.get("GITHUB_RUN_ID")
        else:
            resolved_run_id = os.environ.get("GITHUB_RUN_ID") or event_run_id
        resolved_url = args.run_url or (
            f"{os.environ['GITHUB_SERVER_URL'].rstrip('/')}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{resolved_run_id}"
            if os.environ.get("GITHUB_SERVER_URL")
            and os.environ.get("GITHUB_REPOSITORY")
            and resolved_run_id
            else None
        )
        if args.discover:
            if args.checks is not None:
                raise ExportError("--checks cannot be combined with --discover")
            discovered = discover_github_actions_payload(
                token=args.github_token or os.environ.get("GITHUB_TOKEN"),
                api_url=args.api_url or os.environ.get("GITHUB_API_URL"),
                repository=args.repository or os.environ.get("GITHUB_REPOSITORY"),
                run_id=resolved_run_id,
                collect_evidence=args.collect_evidence or args.download_evidence,
                download_evidence=args.download_evidence,
            )
            payload = build_payload(
                discovered["jobs"],
                {"workflow_run": discovered["run"]},
                run_id=resolved_run_id,
                conclusion=args.conclusion or None,
                run_url=resolved_url,
            )
            discovery_mode = "github_api"
            artifacts = (
                list(discovered.get("artifacts", []))
                if args.collect_evidence or args.download_evidence
                else []
            )
            logs = (
                list(discovered.get("logs", []))
                if args.collect_evidence or args.download_evidence
                else []
            )
            missing_log_locator_count = int(discovered.get("missing_log_locator_count", 0))
            download_stats = discovered.get("download_stats") if args.download_evidence else None
        else:
            if args.checks is None:
                raise ExportError("--checks is required unless --discover is used")
            payload = build_payload(
                _read_json(args.checks, "checks"),
                event,
                run_id=resolved_run_id or None,
                conclusion=args.conclusion or None,
                run_url=resolved_url,
            )
            discovery_mode = "manual"
            artifacts = (
                _artifact_rows(_read_json(args.artifacts, "artifacts"), resolved_run_id)
                if args.artifacts is not None
                else []
            )
            logs = (
                _log_rows(_read_json(args.logs, "logs"), resolved_run_id)
                if args.logs is not None
                else []
            )
            missing_log_locator_count = 0
            download_stats = None
            if args.download_evidence:
                artifacts, logs, download_stats = _download_evidence(
                    artifacts,
                    logs,
                    args.github_token or os.environ.get("GITHUB_TOKEN"),
                )
        attestations = (
            _attestation_rows(_read_json(args.attestations, "attestations"))
            if args.attestations is not None
            else []
        )
        collection = None
        if args.collection_output is not None or args.collect_evidence:
            if args.collection_output is None:
                raise ExportError("--collection-output is required when --collect-evidence is used")
            collection = build_collection_envelope(
                payload,
                artifacts=artifacts,
                logs=logs,
                attestations=attestations,
                discovery_mode=discovery_mode,
                missing_log_locator_count=missing_log_locator_count,
                attestation_mode="caller_supplied" if args.attestations is not None else "not_requested",
                download_stats=download_stats,
            )
        evidence = None
        if args.evidence_output is not None and args.ci is not None:
            evidence = build_provider_evidence_request(
                _read_json(args.ci, "ci"),
                payload,
                artifacts=artifacts,
                logs=logs,
                attestations=attestations,
                source="provider_observed" if discovery_mode == "github_api" else "caller_attested",
            )
        digest = write_payload(
            payload,
            args.output,
            github_output=args.github_output,
            discovery_mode=discovery_mode,
            collection=collection,
            collection_output=args.collection_output,
            evidence=evidence,
            evidence_output=args.evidence_output,
            download_stats=download_stats,
        )
    except ExportError as error:
        print(f"github-actions-evidence: error: {error}", file=sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "payload_path": str(args.output),
                "payload_digest": digest,
                "check_count": len(payload["jobs"]),
                "discovery_mode": discovery_mode,
                "artifact_count": len(artifacts),
                "log_count": len(logs),
                "attestation_count": len(attestations),
                "download_mode": download_stats["mode"] if download_stats is not None else "disabled",
                "downloaded_artifact_count": download_stats["artifact_count"] if download_stats is not None else 0,
                "downloaded_log_count": download_stats["log_count"] if download_stats is not None else 0,
                "downloaded_byte_count": download_stats["total_bytes"] if download_stats is not None else 0,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
