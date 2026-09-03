"""Streaming, deterministic repository manifests for very large trees.

The implementation reads one bounded file buffer at a time and emits bounded chunks. A 5M-line
repository is therefore a scale target, not a requirement to hold 5M path records or file bytes in
memory. Synthetic records model that size without creating millions of files.
"""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable, Iterator

from .errors import ManifestError

READ_BUFFER = 1024 * 1024


@dataclass(frozen=True, order=True)
class FileRecord:
    path: str
    size: int
    lines: int
    digest: str


@dataclass(frozen=True)
class ManifestChunk:
    index: int
    records: tuple[FileRecord, ...]
    digest: str
    bytes: int
    lines: int


@dataclass(frozen=True)
class ManifestSummary:
    root: str
    total_files: int
    total_bytes: int
    total_lines: int
    chunk_count: int
    root_digest: str


def normalize_relative(path: str | os.PathLike[str]) -> str:
    raw = str(path).replace("\\", "/")
    if not raw or raw.startswith("/") or (len(raw) >= 2 and raw[1] == ":"):
        raise ManifestError(f"path must be relative: {path!r}")
    parts = [part for part in raw.split("/") if part not in ("", ".")]
    if not parts or any(part == ".." for part in parts):
        raise ManifestError(f"path escapes the manifest root: {path!r}")
    return "/".join(parts)


def _file_record(root: Path, path: Path, *, max_file_bytes: int | None) -> FileRecord:
    relative = normalize_relative(path.relative_to(root))
    try:
        size = path.stat().st_size
    except OSError as error:
        raise ManifestError(f"cannot stat {relative}: {error}") from error
    if max_file_bytes is not None and size > max_file_bytes:
        raise ManifestError(f"file {relative} is {size} bytes, over the configured limit {max_file_bytes}")
    digest = hashlib.sha256()
    lines = 0
    last_byte: int | None = None
    try:
        with path.open("rb") as handle:
            while True:
                block = handle.read(READ_BUFFER)
                if not block:
                    break
                digest.update(block)
                lines += block.count(b"\n")
                last_byte = block[-1]
    except OSError as error:
        raise ManifestError(f"cannot read {relative}: {error}") from error
    if size and last_byte != ord("\n"):
        lines += 1
    return FileRecord(relative, size, lines, digest.hexdigest())


def iter_file_records(root: str | os.PathLike[str], *, max_file_bytes: int | None = None) -> Iterator[FileRecord]:
    base = Path(root).resolve()
    if not base.is_dir():
        raise ManifestError(f"manifest root is not a directory: {root}")
    for current, directories, names in os.walk(base, topdown=True, followlinks=False):
        directories[:] = sorted(name for name in directories if not (Path(current) / name).is_symlink())
        for name in sorted(names):
            path = Path(current) / name
            if path.is_symlink() or not path.is_file():
                continue
            yield _file_record(base, path, max_file_bytes=max_file_bytes)


def _record_bytes(record: FileRecord) -> bytes:
    return (json.dumps(asdict(record), sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def _make_chunk(index: int, records: tuple[FileRecord, ...]) -> ManifestChunk:
    encoded = b"".join(_record_bytes(record) for record in records)
    return ManifestChunk(index, records, hashlib.sha256(encoded).hexdigest(), sum(r.size for r in records), sum(r.lines for r in records))


def chunks_from_records(records: Iterable[FileRecord], *, chunk_records: int = 1024) -> Iterator[ManifestChunk]:
    if chunk_records <= 0:
        raise ManifestError("chunk_records must be positive")
    bucket: list[FileRecord] = []
    index = 0
    for record in records:
        bucket.append(record)
        if len(bucket) == chunk_records:
            yield _make_chunk(index, tuple(bucket))
            index += 1
            bucket.clear()
    if bucket:
        yield _make_chunk(index, tuple(bucket))


def stream_manifest(root: str | os.PathLike[str], *, chunk_records: int = 1024, max_file_bytes: int | None = None) -> Iterator[ManifestChunk]:
    return chunks_from_records(iter_file_records(root, max_file_bytes=max_file_bytes), chunk_records=chunk_records)


def summarize(root: str, chunks: Iterable[ManifestChunk]) -> ManifestSummary:
    root_hash = hashlib.sha256()
    total_files = total_bytes = total_lines = chunk_count = 0
    for chunk in chunks:
        root_hash.update(bytes.fromhex(chunk.digest))
        total_files += len(chunk.records)
        total_bytes += chunk.bytes
        total_lines += chunk.lines
        chunk_count += 1
    return ManifestSummary(
        root=root,
        total_files=total_files,
        total_bytes=total_bytes,
        total_lines=total_lines,
        chunk_count=chunk_count,
        root_digest=root_hash.hexdigest(),
    )


def synthetic_records(*, file_count: int, lines_per_file: int, bytes_per_line: int = 80) -> Iterator[FileRecord]:
    """Yield logical records for scale tests; it never creates files or line-sized buffers."""
    if file_count < 0 or lines_per_file < 0 or bytes_per_line <= 0:
        raise ManifestError("synthetic dimensions must be nonnegative with positive bytes_per_line")
    for index in range(file_count):
        path = f"synthetic/module-{index:08d}.src"
        logical = f"synthetic:{path}:{lines_per_file}:{bytes_per_line}".encode()
        digest = hashlib.sha256(logical).hexdigest()
        yield FileRecord(path, lines_per_file * bytes_per_line, lines_per_file, digest)
