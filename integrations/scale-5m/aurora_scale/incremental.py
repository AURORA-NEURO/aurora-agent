"""Incremental digest comparison over bounded manifest records."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Iterable

from .manifest import FileRecord


@dataclass(frozen=True)
class ChangeSet:
    added: tuple[str, ...]
    removed: tuple[str, ...]
    changed: tuple[str, ...]
    unchanged: int
    digest: str


def incremental_changes(previous: Iterable[FileRecord], current: Iterable[FileRecord]) -> ChangeSet:
    old = {record.path: record.digest for record in previous}
    new = {record.path: record.digest for record in current}
    added = tuple(sorted(set(new) - set(old)))
    removed = tuple(sorted(set(old) - set(new)))
    changed = tuple(sorted(path for path in set(old) & set(new) if old[path] != new[path]))
    unchanged = sum(1 for path in set(old) & set(new) if old[path] == new[path])
    payload = {"added": added, "changed": changed, "removed": removed, "unchanged": unchanged}
    digest = hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return ChangeSet(added, removed, changed, unchanged, digest)
