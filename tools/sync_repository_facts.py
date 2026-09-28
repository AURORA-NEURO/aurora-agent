#!/usr/bin/env python3
"""Synchronize public workspace facts with the Cargo workspace and MCP catalogue.

Run with ``--write`` after changing the workspace or MCP catalogue. The default
``--check`` mode is suitable for CI and reports documentation that has drifted.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FACT_PATTERNS = {
    "crates": re.compile(r"(?<![\w.])[\d][\d,]*(?= crates\b)"),
    "tools": re.compile(
        r"(?<![\w.])[\d][\d,]*(?= (?:(?:MCP|callable) )?tools\b)"
    ),
    "rust_lines": re.compile(
        r"(?<=, )[\d,]+(?= lines, clippy -D warnings enforced in CI\.\*\*)"
    ),
}

# These are user-facing statements of current workspace and catalogue size.
# Historical numbers, benchmark counts, and test assertions are intentionally
# not included.
DOCUMENT_FACTS = {
    "README.md": ("crates", "rust_lines", "tools"),
    "CONTRIBUTING.md": ("crates",),
    "CITATION.cff": ("crates", "tools"),
    "Dockerfile": ("tools",),
    "llms.txt": ("crates", "tools"),
    "site/llms.txt": ("crates", "tools"),
    "site/llms-full.txt": ("crates", "tools"),
    "site/install.html": ("tools",),
    "site/index.html": ("crates", "tools"),
    "site/faq.html": ("crates", "tools"),
    "plugins/aurora-backend/skills/aurora-backend-setup/SKILL.md": ("tools",),
    "docs/ARCHITECTURE.md": ("crates",),
    "docs/COVERAGE.md": ("tools",),
    "docs/ISSUE_REPAIR.md": ("tools",),
}


def repository_facts() -> dict[str, int]:
    metadata = subprocess.run(
        [
            "cargo",
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--locked",
            "--offline",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    workspace = json.loads(metadata.stdout)
    catalogue = json.loads(
        (ROOT / "crates/mcp/src/tool_definitions.json").read_text(encoding="utf-8")
    )
    if not isinstance(catalogue, list):
        raise ValueError("embedded MCP catalogue must be a JSON array")
    return {
        "crates": len(workspace["workspace_members"]),
        "tools": len(catalogue),
        # Match `wc -l` used by tools/status.sh: count newline bytes in Rust sources.
        "rust_lines": sum(
            path.read_bytes().count(b"\n")
            for path in (ROOT / "crates").rglob("*.rs")
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check", action="store_true", help="report drift without writing (default)"
    )
    mode.add_argument(
        "--write", action="store_true", help="update documented facts in place"
    )
    args = parser.parse_args()
    facts = repository_facts()
    updates: dict[str, str] = {}
    originals: dict[str, str] = {}
    problems: list[str] = []

    for relative_path, fact_names in DOCUMENT_FACTS.items():
        path = ROOT / relative_path
        original = path.read_bytes().decode("utf-8")
        originals[relative_path] = original
        updated = original
        for fact_name in fact_names:
            pattern = FACT_PATTERNS[fact_name]
            if not pattern.search(updated):
                problems.append(f"{relative_path} has no documented {fact_name} marker")
                continue
            updated = pattern.sub(f"{facts[fact_name]:,}", updated)
        updates[relative_path] = updated

    if problems:
        print("Repository fact synchronization could not validate all markers:", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 2

    stale = [
        relative_path
        for relative_path, updated in updates.items()
        if updated != originals[relative_path]
    ]
    if args.write:
        for relative_path in stale:
            (ROOT / relative_path).write_bytes(updates[relative_path].encode("utf-8"))

    if args.write:
        print(
            f"Synchronized repository facts in {len(stale)} file(s): "
            f"{', '.join(stale) or 'none'}"
        )
        print(
            f"Workspace crates: {facts['crates']}; MCP tools: {facts['tools']}; "
            f"Rust source lines: {facts['rust_lines']:,}"
        )
        return 0
    if stale:
        print("Repository facts are stale in: " + ", ".join(stale), file=sys.stderr)
        print(
            "Run `python tools/sync_repository_facts.py --write` to synchronize them.",
            file=sys.stderr,
        )
        return 1
    print(
        f"Repository facts are current: {facts['crates']} crates, {facts['tools']} MCP tools, "
        f"{facts['rust_lines']:,} Rust source lines"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"repository fact synchronization failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
