"""Regenerates the golden artifacts the Rust parity tests compare against.

The Rust compiler must emit bytes identical to the CPython reference runtime, so the reference
is the source of truth for these files. Run this after changing anything in
reference/fiber_runtime/, then re-run `cargo test` and inspect the diff: a change here is a
change to the wire format and needs a schema version bump.

Usage: python tools/regenerate_golden.py
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures" / "fiber-v0.1"
GOLDEN = FIXTURES / "golden"


def canonical(obj) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def main() -> int:
    GOLDEN.mkdir(parents=True, exist_ok=True)
    certificate = GOLDEN / "reference_certificate.json"
    section = GOLDEN / "reference_section.json"

    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "reference" / "fiber_runtime" / "fiber_compile.py"),
            "--world", str(FIXTURES / "radiogenomic_world.json"),
            "--query", str(FIXTURES / "leakage_query.json"),
            "--output", str(certificate),
            "--section-output", str(section),
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        return result.returncode

    print(result.stdout.strip())
    print()
    for path in (FIXTURES / "radiogenomic_world.json", FIXTURES / "leakage_query.json", section, certificate):
        digest = hashlib.sha256(canonical(json.loads(path.read_text(encoding="utf-8")))).hexdigest()
        print(f"{digest}  {path.relative_to(ROOT).as_posix()}")

    body = json.loads(certificate.read_text(encoding="utf-8"))
    claimed = body.pop("certificate_sha256")
    recomputed = hashlib.sha256(canonical(body)).hexdigest()
    print()
    print(f"certificate_sha256 claimed    : {claimed}")
    print(f"certificate_sha256 recomputed : {recomputed}")
    if claimed != recomputed:
        print("MISMATCH: the reference runtime's own digest does not verify", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
