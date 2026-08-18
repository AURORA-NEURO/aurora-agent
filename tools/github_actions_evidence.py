#!/usr/bin/env python3
"""Export a bounded GitHub Actions run into Aurora's provider-payload shape.

The exporter has two explicit modes.  Manual mode reads caller-selected rows from a local JSON
file.  Discovery mode uses a caller-supplied GitHub token to retrieve one run and its bounded job
list through the GitHub API.  Both modes are ingestion helpers, not evidence verifiers: the output
contains no token, does not download logs or artifacts, and leaves result-digest derivation and
release decisions to the Rust ``ci_provider_normalize``/``ci_provider_evidence_audit`` contracts.
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
from urllib.request import Request, urlopen


MAX_INPUT_BYTES = 2 * 1024 * 1024
MAX_CHECKS = 64
MAX_TEXT_BYTES = 512
MAX_DURATION_MS = 7 * 24 * 60 * 60 * 1000
MAX_API_RESPONSE_BYTES = 2 * 1024 * 1024
MAX_DISCOVERY_PAGE_SIZE = MAX_CHECKS + 1
MAX_DISCOVERY_TIMEOUT_SECONDS = 30
SCHEMA = "bioprism-actions-github-provider-payload/0.1"


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
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "aurora-agent-github-actions-evidence",
        },
        method="GET",
    )
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


def discover_github_actions_payload(
    *,
    token: Any,
    api_url: Any,
    repository: Any,
    run_id: Any,
) -> dict[str, Any]:
    """Retrieve one bounded GitHub run and convert its jobs to exporter input rows.

    The API response is treated as an observed provider payload, not as a signature or log proof.
    A response larger than the normalizer's check bound is refused instead of truncated, because a
    partial job list could make a required check look absent for the wrong reason.
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
    return {"run": dict(run), "jobs": rows}


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


def write_payload(
    payload: Mapping[str, Any],
    output: Path,
    *,
    github_output: Path | None = None,
    discovery_mode: str = "manual",
) -> str:
    """Write deterministic JSON and optional GitHub Action outputs."""

    output = _validated_path("output path", output)
    if github_output is not None:
        github_output = _validated_path("GITHUB_OUTPUT path", github_output)
    if discovery_mode not in {"manual", "github_api"}:
        raise ExportError("discovery_mode must be manual or github_api")
    encoded = canonical_bytes(payload)
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(encoded + b"\n")
    except OSError as error:
        raise ExportError(f"cannot write output {output}: {error}") from error
    digest = hashlib.sha256(encoded).hexdigest()
    if github_output is not None:
        try:
            with github_output.open("a", encoding="utf-8", newline="\n") as stream:
                stream.write(f"payload-path={output}\n")
                stream.write(f"payload-digest={digest}\n")
                stream.write(f"run-id={payload['run']['id']}\n")
                stream.write(f"check-count={len(payload['jobs'])}\n")
                stream.write(f"discovery-mode={discovery_mode}\n")
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
    parser.add_argument("--github-token", default="", help="token for bounded GitHub API discovery")
    parser.add_argument("--api-url", default="", help="GitHub API base URL for discovery")
    parser.add_argument("--repository", default="", help="owner/name repository for discovery")
    parser.add_argument(
        "--event",
        type=_optional_path,
        help="optional GitHub event JSON; defaults to GITHUB_EVENT_PATH",
    )
    parser.add_argument("--run-id", default="", help="optional run id; defaults to event/GITHUB_RUN_ID")
    parser.add_argument("--conclusion", default="", help="optional conclusion; defaults to event metadata")
    parser.add_argument("--run-url", default="", help="optional run URL; defaults to event metadata")
    parser.add_argument("--output", required=True, type=Path, help="deterministic provider payload output path")
    parser.add_argument("--github-output", type=Path, help="optional GITHUB_OUTPUT file")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
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
            )
            payload = build_payload(
                discovered["jobs"],
                {"workflow_run": discovered["run"]},
                run_id=resolved_run_id,
                conclusion=args.conclusion or None,
                run_url=resolved_url,
            )
            discovery_mode = "github_api"
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
        digest = write_payload(
            payload,
            args.output,
            github_output=args.github_output,
            discovery_mode=discovery_mode,
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
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
