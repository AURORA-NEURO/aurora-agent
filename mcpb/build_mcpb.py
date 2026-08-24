#!/usr/bin/env python3
"""Assemble and pack the aurora-agent Claude Desktop extension (.mcpb).

Stages the release-built MCP binary plus a minimal confined root (reference
fixtures and schemas) next to the manifest, then packs with the official CLI:

    python mcpb/build_mcpb.py            # stages + packs mcpb/dist/aurora-agent.mcpb

Requires: target/release/bioprism-mcp.exe (cargo build --release --offline
-p bioprism-mcp) and npx (the script shells out to @anthropic-ai/mcpb).
"""

from pathlib import Path
import json
import os
import shutil
import subprocess
import sys

REPO = Path(__file__).resolve().parent.parent
STAGE = REPO / "mcpb" / "stage"
DIST = REPO / "mcpb" / "dist"
EXE = REPO / "target" / "release" / "bioprism-mcp.exe"

ROOT_CONTENT = [
    "fixtures/fiber-v0.1",
    "fixtures/fiber-v0.3",
    "fixtures/fiber-v0.4",
    "fixtures/fiber-v0.5",
    "fixtures/generated",
    "schemas",
]


def main() -> int:
    if not EXE.is_file():
        print(f"missing binary: {EXE}\nbuild it: cargo build --release --offline -p bioprism-mcp", file=sys.stderr)
        return 2
    if STAGE.exists():
        shutil.rmtree(STAGE)
    (STAGE / "server").mkdir(parents=True)
    manifest = json.loads((REPO / "mcpb" / "manifest.json").read_text(encoding="utf-8"))
    version = os.environ.get("MCPB_VERSION")
    if version:
        manifest["version"] = version.removeprefix("v")
    (STAGE / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    shutil.copy2(EXE, STAGE / "server" / "bioprism-mcp.exe")
    for rel in ROOT_CONTENT:
        src = REPO / rel
        if src.is_dir():
            shutil.copytree(src, STAGE / "root" / rel)
    DIST.mkdir(parents=True, exist_ok=True)
    out = DIST / "aurora-agent.mcpb"
    npx = shutil.which("npx") or shutil.which("npx.cmd") or "npx"
    for cmd in (
        [npx, "--yes", "@anthropic-ai/mcpb", "validate", str(STAGE / "manifest.json")],
        [npx, "--yes", "@anthropic-ai/mcpb", "pack", str(STAGE), str(out)],
    ):
        result = subprocess.run(cmd, cwd=REPO)
        if result.returncode != 0:
            return result.returncode
    print(f"packed: {out} ({out.stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
