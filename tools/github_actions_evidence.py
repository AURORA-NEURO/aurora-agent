#!/usr/bin/env python3
"""Export a bounded GitHub Actions run into Aurora's provider-payload shape.

The exporter is deliberately an ingestion helper, not an evidence verifier.  It reads only the
selected run metadata and job/check rows, writes a deterministic JSON payload, and leaves provider
authentication, log retrieval, result-digest derivation, and release decisions to the Rust
``ci_provider_normalize``/``ci_provider_evidence_audit`` contracts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any, Mapping, Sequence


MAX_INPUT_BYTES = 2 * 1024 * 1024
MAX_CHECKS = 64
MAX_TEXT_BYTES = 512
MAX_DURATION_MS = 7 * 24 * 60 * 60 * 1000
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
) -> str:
    """Write deterministic JSON and optional GitHub Action outputs."""

    output = _validated_path("output path", output)
    if github_output is not None:
        github_output = _validated_path("GITHUB_OUTPUT path", github_output)
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
        except OSError as error:
            raise ExportError(f"cannot write GITHUB_OUTPUT {github_output}: {error}") from error
    return digest


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checks", required=True, type=Path, help="JSON array/object containing bounded job rows")
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
        resolved_run_id = args.run_id or os.environ.get("GITHUB_RUN_ID")
        resolved_url = args.run_url or (
            f"{os.environ['GITHUB_SERVER_URL'].rstrip('/')}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{resolved_run_id}"
            if os.environ.get("GITHUB_SERVER_URL")
            and os.environ.get("GITHUB_REPOSITORY")
            and resolved_run_id
            else None
        )
        payload = build_payload(
            _read_json(args.checks, "checks"),
            event,
            run_id=resolved_run_id or None,
            conclusion=args.conclusion or None,
            run_url=resolved_url,
        )
        digest = write_payload(payload, args.output, github_output=args.github_output)
    except ExportError as error:
        print(f"github-actions-evidence: error: {error}", file=sys.stderr)
        return 2
    print(json.dumps({"payload_path": str(args.output), "payload_digest": digest, "check_count": len(payload["jobs"])}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
