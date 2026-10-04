#!/usr/bin/env python3
"""Keep the topic-grouped documentation map complete and its local links live."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "docs" / "README.md"
LINK = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")


def validate() -> int:
    if not INDEX.is_file():
        raise ValueError("docs/README.md is missing")
    text = INDEX.read_text(encoding="utf-8")
    linked_files: set[Path] = set()
    broken: list[str] = []

    for raw_target in LINK.findall(text):
        target = urlsplit(raw_target.strip())
        if target.scheme or target.netloc:
            continue
        path_text = unquote(target.path)
        if not path_text:
            continue
        resolved = (INDEX.parent / path_text).resolve()
        try:
            resolved.relative_to(ROOT)
        except ValueError:
            broken.append(raw_target)
            continue
        if not resolved.is_file():
            broken.append(raw_target)
            continue
        linked_files.add(resolved)

    expected = {
        path.resolve()
        for path in (ROOT / "docs").glob("*.md")
        if path.name != INDEX.name
    }
    missing = sorted(path.relative_to(ROOT).as_posix() for path in expected - linked_files)
    if broken or missing:
        details = []
        if broken:
            details.append("broken or out-of-repository links: " + ", ".join(broken))
        if missing:
            details.append("top-level references absent from the map: " + ", ".join(missing))
        raise ValueError("documentation index is incomplete: " + "; ".join(details))

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if not re.search(r"\[[^\]]+\]\(docs/README\.md(?:#[^)]*)?\)", readme):
        raise ValueError("root README.md must link the documentation map")
    return len(expected)


if __name__ == "__main__":
    try:
        count = validate()
    except (OSError, UnicodeError, ValueError) as error:
        print(f"documentation index validation failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
    print(f"documentation index valid: {count} top-level references linked")
