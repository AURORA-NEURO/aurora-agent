#!/usr/bin/env python3
"""Generate or validate the complete direct Rust workspace dependency index."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/WORKSPACE_DEPENDENCIES.md"
DEPENDENCY_KINDS = {
    "normal": "Normal/runtime",
    "build": "Build-time",
    "dev": "Development/test",
}


def load_workspace() -> list[dict]:
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
    value = json.loads(metadata.stdout)
    member_ids = set(value["workspace_members"])
    packages = [package for package in value["packages"] if package["id"] in member_ids]
    if len(packages) != len(member_ids):
        raise ValueError("Cargo metadata omitted one or more workspace packages")
    return sorted(packages, key=lambda package: package["name"])


def markdown_text(value: str) -> str:
    return value.replace("|", r"\|").replace("`", r"\`").replace("\n", " ")


def package_link(package: dict) -> str:
    manifest = Path(package["manifest_path"])
    try:
        relative = manifest.relative_to(ROOT).parent.as_posix()
    except ValueError as error:
        raise ValueError(
            f"workspace package {package['name']} has a manifest outside the checkout"
        ) from error
    return f"[`{markdown_text(package['name'])}`](../{relative}/Cargo.toml)"


def dependency_link(dependency: dict, package_by_name: dict[str, dict]) -> str | None:
    package = package_by_name.get(dependency["name"])
    if package is None:
        return None
    label = markdown_text(dependency.get("rename") or package["name"])
    rendered = f"[`{label}`](../{Path(package['manifest_path']).relative_to(ROOT).parent.as_posix()}/Cargo.toml)"
    if dependency.get("rename"):
        rendered += f" → {package_link(package)}"
    details = []
    if dependency.get("optional"):
        details.append("optional")
    if not dependency.get("uses_default_features", True):
        details.append("default features off")
    features = sorted(dependency.get("features", []))
    if features:
        details.append("features: " + ", ".join(f"`{markdown_text(feature)}`" for feature in features))
    target = dependency.get("target")
    if target:
        details.append("target: `" + markdown_text(target) + "`")
    if details:
        rendered += " (" + "; ".join(details) + ")"
    return rendered


def render() -> str:
    packages = load_workspace()
    package_by_name = {package["name"]: package for package in packages}
    edges_by_kind: Counter[str] = Counter()
    rows = []

    for package in packages:
        dependencies_by_kind = {kind: [] for kind in DEPENDENCY_KINDS}
        for dependency in package["dependencies"]:
            kind = dependency.get("kind") or "normal"
            if kind not in DEPENDENCY_KINDS:
                raise ValueError(
                    f"unknown Cargo dependency kind {kind!r} in {package['name']}"
                )
            rendered = dependency_link(dependency, package_by_name)
            if rendered is None:
                continue
            dependencies_by_kind[kind].append(rendered)
            edges_by_kind[kind] += 1

        for dependencies in dependencies_by_kind.values():
            dependencies.sort()
        cells = [
            ", ".join(dependencies_by_kind[kind]) or "—"
            for kind in DEPENDENCY_KINDS
        ]
        rows.append(f"| {package_link(package)} | " + " | ".join(cells) + " |")

    edge_count = sum(edges_by_kind.values())
    lines = [
        "# Rust workspace dependency index",
        "",
        "This index lists direct dependencies between packages in this Cargo workspace. Cargo manifests",
        "remain authoritative; external registry dependencies are omitted. Optional dependencies,",
        "explicit feature selections, disabled default features, and target conditions are marked inline.",
        "Normal dependencies are available to the package's library or binary; build-time and",
        "development/test dependencies are shown in their own columns.",
        "",
        f"Generated from `cargo metadata --no-deps --format-version 1 --locked --offline`: "
        f"**{len(packages)} packages**, **{edge_count} direct workspace dependency declarations** "
        "("
        + ", ".join(
            f"{edges_by_kind[kind]} {kind}" for kind in DEPENDENCY_KINDS
        )
        + ").",
        "",
        "Regenerate with `python tools/sync_workspace_dependencies.py --write`; CI checks the "
        "committed index with `--check`.",
        "",
        "| Workspace package | Normal/runtime dependencies | Build-time dependencies | Development/test dependencies |",
        "|---|---|---|---|",
        *rows,
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="report drift without writing (default)")
    mode.add_argument("--write", action="store_true", help="write the generated index")
    args = parser.parse_args()
    expected = render()
    current = OUTPUT.read_bytes().decode("utf-8") if OUTPUT.exists() else None
    if current == expected:
        print("Workspace dependency index is current")
        return 0
    if args.write:
        OUTPUT.write_bytes(expected.encode("utf-8"))
        print(f"Generated {OUTPUT.relative_to(ROOT)}")
        return 0
    print(
        "Workspace dependency index is missing or stale; run "
        "`python tools/sync_workspace_dependencies.py --write`.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"workspace dependency index failed: {error}", file=sys.stderr)
        raise SystemExit(2) from error
