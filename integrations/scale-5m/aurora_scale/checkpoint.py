"""Canonical, resumable manifest checkpoints."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path

from .errors import CheckpointError


@dataclass(frozen=True)
class ManifestCheckpoint:
    root: str
    next_chunk: int
    last_chunk_digest: str | None
    root_digest: str | None = None


def encode_checkpoint(checkpoint: ManifestCheckpoint) -> str:
    if checkpoint.next_chunk < 0:
        raise CheckpointError("next_chunk cannot be negative")
    return json.dumps(asdict(checkpoint), sort_keys=True, separators=(",", ":"))


def decode_checkpoint(text: str) -> ManifestCheckpoint:
    try:
        value = json.loads(text)
        checkpoint = ManifestCheckpoint(**value)
    except (ValueError, TypeError, json.JSONDecodeError) as error:
        raise CheckpointError("checkpoint is not valid JSON with the required fields") from error
    if checkpoint.next_chunk < 0:
        raise CheckpointError("next_chunk cannot be negative")
    return checkpoint


def write_checkpoint(path: str | Path, checkpoint: ManifestCheckpoint) -> None:
    target = Path(path)
    target.write_text(encode_checkpoint(checkpoint), encoding="utf-8")


def read_checkpoint(path: str | Path) -> ManifestCheckpoint:
    try:
        return decode_checkpoint(Path(path).read_text(encoding="utf-8"))
    except OSError as error:
        raise CheckpointError(f"cannot read checkpoint: {error}") from error
