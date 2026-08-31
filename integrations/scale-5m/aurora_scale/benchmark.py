"""Synthetic scale benchmark with a declared memory model."""

from __future__ import annotations

import time

from .manifest import chunks_from_records, synthetic_records


def synthetic_manifest_benchmark(*, file_count: int, lines_per_file: int, chunk_records: int = 1024) -> dict[str, int | float | str]:
    start = time.perf_counter()
    chunks = 0
    files = 0
    lines = 0
    for chunk in chunks_from_records(synthetic_records(file_count=file_count, lines_per_file=lines_per_file), chunk_records=chunk_records):
        chunks += 1
        files += len(chunk.records)
        lines += chunk.lines
    elapsed = max(time.perf_counter() - start, 1e-9)
    # This is a bound model for record/chunk state, not an OS RSS claim.
    peak_record_state = min(file_count, chunk_records)
    return {
        "files": files,
        "lines": lines,
        "chunks": chunks,
        "chunk_records": chunk_records,
        "peak_record_state": peak_record_state,
        "records_per_second": files / elapsed if files else 0.0,
        "model": "synthetic-logical-records; no files created",
    }
