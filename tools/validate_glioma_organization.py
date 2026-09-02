#!/usr/bin/env python3
"""Validate the executable glioma product's folder-by-folder organization contract.

The sibling ``aurora-feature-atlas`` owns the 79-crate/80,896-feature portfolio.  This validator
keeps the smaller executable slice honest: twelve owned program folders, thirty-two generated
feature slots per program, explicit source roots, and a stable implementation manifest.  It is
deliberately dependency-free so it can run in a release checkout and in CI before an algorithm is
added to a program.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import NoReturn


PROGRAMS = {
    "P01": "p01_evidence_surveillance",
    "P02": "p02_evidence_knowledge",
    "P03": "p03_multimodal_ingestion_qc",
    "P04": "p04_decision_context",
    "P05": "p05_mechanism_exploration",
    "P06": "p06_experiment_design",
    "P07": "p07_protocol_simulation",
    "P08": "p08_instrument_robotics",
    "P09": "p09_reproducible_computation",
    "P10": "p10_interpretation_replication",
    "P11": "p11_research_object_release",
    "P12": "p12_federated_benchmarking",
}


def fail(message: str) -> "NoReturn":
    raise ValueError(message)


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")


def validate(root: Path) -> dict:
    organization_path = root / "docs" / "glioma" / "organization.json"
    organization = load_json(organization_path)
    if organization.get("program_count") != 12:
        fail("organization.json must declare exactly 12 executable programs")
    if organization.get("features_per_program") != 32:
        fail("organization.json must declare exactly 32 feature slots per program")
    if organization.get("feature_count") != 384:
        fail("organization.json must declare exactly 384 executable feature slots")

    roots = organization.get("roots", {})
    for key in ("engine", "programs", "contracts", "mcp", "organization_validator"):
        value = roots.get(key)
        if not isinstance(value, str) or not value.strip():
            fail(f"organization root {key!r} is missing")
        if key != "organization_validator" and not (root / value).exists():
            fail(f"organization root {key!r} does not exist: {value}")

    program_rows = organization.get("programs")
    if not isinstance(program_rows, list) or len(program_rows) != len(PROGRAMS):
        fail("organization.json program rows do not match the twelve-program contract")
    rows_by_id = {row.get("id"): row for row in program_rows}
    if set(rows_by_id) != set(PROGRAMS):
        fail("organization.json program ids do not match P01–P12")

    source_root = root / "crates" / "research" / "src" / "glioma" / "programs"
    catalog_path = root / "crates" / "research" / "src" / "glioma" / "catalog.rs"
    if not catalog_path.is_file():
        fail(f"catalog source is missing: {catalog_path}")
    contract_root = root / "crates" / "research" / "src" / "glioma"
    implemented_ids = sorted(
        {
            match.group(1)
            for path in contract_root.rglob("*.rs")
            for match in re.finditer(
                r"pub\s+const\s+FEATURE_ID:\s*&str\s*=\s*\"(GAF-GLIOMA-P\d{2}-F\d{2})\"",
                path.read_text(encoding="utf-8"),
            )
        }
    )
    if not implemented_ids:
        fail("catalog.rs does not expose an implementation manifest")
    if len(implemented_ids) != len(set(implemented_ids)):
        fail("implementation ids are duplicated")

    folder_report = []
    for program_id, folder_name in PROGRAMS.items():
        row = rows_by_id[program_id]
        folder = source_root / folder_name
        if not folder.is_dir():
            fail(f"{program_id} source folder is missing: {folder}")
        if not (folder / "mod.rs").is_file():
            fail(f"{program_id} source folder has no mod.rs ownership boundary")
        if row.get("folder") != folder_name:
            fail(f"{program_id} folder mismatch between organization.json and source tree")
        surfaces = row.get("surfaces", [])
        if not isinstance(surfaces, list) or any(not isinstance(surface, str) for surface in surfaces):
            fail(f"{program_id} surfaces must be a list of named product routes")
        feature_prefix = f"GAF-GLIOMA-{program_id}-F"
        program_ids = [feature_id for feature_id in implemented_ids if feature_id.startswith(feature_prefix)]
        folder_report.append(
            {
                "program_id": program_id,
                "source_folder": f"crates/research/src/glioma/programs/{folder_name}",
                "feature_slot_count": 32,
                "implemented_feature_count": len(program_ids),
                "implemented_feature_ids": program_ids,
                "surfaces": surfaces,
                "ownership_boundary": f"{folder_name}/mod.rs",
            }
        )

    expected_total = organization["program_count"] * organization["features_per_program"]
    if expected_total != organization["feature_count"]:
        fail("organization feature cardinality is not multiplicative")
    return {
        "schema_version": "glioma-folder-organization/1.0",
        "program_count": len(folder_report),
        "feature_slot_count": expected_total,
        "implemented_feature_count": len(implemented_ids),
        "planned_feature_count": expected_total - len(implemented_ids),
        "programs": folder_report,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--json", action="store_true", help="emit a machine-readable report")
    args = parser.parse_args()
    try:
        report = validate(args.root.resolve())
    except ValueError as error:
        print(f"glioma organization validation failed: {error}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(
            "glioma organization valid: "
            f"{report['program_count']} programs, "
            f"{report['feature_slot_count']} feature slots, "
            f"{report['implemented_feature_count']} implemented"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
