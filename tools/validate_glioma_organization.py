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


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=unique_object
        )
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")


def module_doc(text: str, path: Path) -> str:
    """Return the module's leading Rust-doc paragraph as its human-owned purpose statement."""
    lines = []
    for line in text.splitlines():
        if not line.startswith("//!"):
            break
        lines.append(line[3:].strip())
    while lines and not lines[0]:
        lines.pop(0)
    paragraph = []
    for line in lines:
        if not line:
            break
        paragraph.append(line)
    if not paragraph:
        fail(f"Rust module has no leading //! purpose paragraph: {path}")
    return " ".join(paragraph)


def module_inventory(root: Path, folder: Path, program_id: str) -> list[dict]:
    """Inventory every file-owned module and distinguish features from composition helpers."""
    ownership = (folder / "mod.rs").read_text(encoding="utf-8")
    rows = []
    for path in sorted(folder.glob("*.rs"), key=lambda item: item.name):
        if path.name == "mod.rs":
            continue
        text = path.read_text(encoding="utf-8")
        purpose = module_doc(text, path)
        const = re.search(
            r'pub\s+const\s+FEATURE_ID:\s*&str\s*=\s*([^;]+);', text
        )
        raw_feature = const.group(1).strip() if const else None
        feature_id = None
        feature_alias = None
        if raw_feature and raw_feature.startswith('"') and raw_feature.endswith('"'):
            feature_id = raw_feature[1:-1]
        elif raw_feature:
            feature_alias = raw_feature
        if feature_id and not feature_id.startswith(f"GAF-GLIOMA-{program_id}-F"):
            fail(
                f"{program_id} module {path.name} owns a feature from another program: "
                f"{feature_id}"
            )
        export_pattern = rf"(?m)^\s*pub\s+mod\s+{re.escape(path.stem)}\s*;"
        exported = re.search(export_pattern, ownership) is not None
        if not exported:
            fail(f"Rust module is not exported by its folder ownership boundary: {path}")
        local_refs = set(
            re.findall(r"\bsuper::(?!super::)([a-z][a-z0-9_]*)::", text)
        )
        external_refs = set(
            re.findall(
                r"crate::glioma::programs::(p[0-9]{2}_[a-z0-9_]+)::([a-z][a-z0-9_]*)::",
                text,
            )
        )
        dependencies = sorted(
            local_refs
            | {
                f"{program}::{module}"
                for program, module in external_refs
                if program != folder.name
            }
        )
        rows.append(
            {
                "module": path.stem,
                "path": path.relative_to(root).as_posix(),
                "purpose": purpose,
                "kind": (
                    "feature"
                    if feature_id
                    else "feature_alias"
                    if feature_alias
                    else "workflow_composition"
                ),
                "feature_id": feature_id,
                "feature_alias": feature_alias,
                "test_annotations": len(
                    re.findall(r"#\s*\[\s*(?:tokio::)?test\s*\]", text)
                ),
                "dependencies": dependencies,
                "exported": exported,
            }
        )
    return rows


def shared_module_inventory(root: Path, contract_root: Path) -> list[dict]:
    """List public feature modules kept at the historical shared glioma API root."""
    ownership = (contract_root / "mod.rs").read_text(encoding="utf-8")
    rows = []
    for path in sorted(contract_root.glob("*.rs"), key=lambda item: item.name):
        if path.name in ("mod.rs", "catalog.rs"):
            continue
        text = path.read_text(encoding="utf-8")
        const = re.search(
            r'pub\s+const\s+FEATURE_ID:\s*&str\s*=\s*([^;]+);', text
        )
        if not const:
            continue
        raw_feature = const.group(1).strip()
        feature_id = (
            raw_feature[1:-1]
            if raw_feature.startswith('"') and raw_feature.endswith('"')
            else None
        )
        feature_alias = None if feature_id else raw_feature
        export_pattern = rf"(?m)^\s*pub\s+mod\s+{re.escape(path.stem)}\s*;"
        exported = re.search(export_pattern, ownership) is not None
        if not exported:
            fail(f"shared glioma module is not exported by glioma/mod.rs: {path}")
        target = feature_id or feature_alias or ""
        program_id = None
        program = re.search(r"GAF-GLIOMA-(P[0-9]{2})-F[0-9]{2}", target)
        if not program and feature_alias:
            program = re.search(r"p([0-9]{2})_[a-z0-9_]+::", feature_alias)
            if program:
                program_id = f"P{program.group(1)}"
        else:
            program_id = program.group(1) if program else None
        if feature_id and not program_id:
            fail(f"shared module has a malformed stable feature ID: {path}")
        rows.append(
            {
                "module": path.stem,
                "path": path.relative_to(root).as_posix(),
                "purpose": module_doc(text, path),
                "kind": "feature" if feature_id else "feature_alias",
                "feature_id": feature_id,
                "feature_alias": feature_alias,
                "program_id": program_id,
                "test_annotations": len(
                    re.findall(r"#\s*\[\s*(?:tokio::)?test\s*\]", text)
                ),
                "exported": exported,
            }
        )
    return rows


def plan_charter(plan_text: str, program_id: str) -> str:
    headings = list(re.finditer(r"(?m)^### (P[0-9]{2}) — `([^`]+)`\s*$", plan_text))
    for index, heading in enumerate(headings):
        if heading.group(1) != program_id:
            continue
        end = headings[index + 1].start() if index + 1 < len(headings) else len(plan_text)
        next_section = re.search(r"(?m)^## ", plan_text[heading.end() : end])
        if next_section:
            end = heading.end() + next_section.start()
        return plan_text[heading.start() : end].strip()
    fail(f"program plan has no folder charter for {program_id}")


def validate_feature_backlog(
    root: Path, path: Path, expected_ids: set[str], all_ids: set[str]
) -> list[dict]:
    """Require every unimplemented slot to have a product-ready folder-level design."""
    backlog_files = sorted(path.glob("P[0-9][0-9]*.json")) if path.is_dir() else [path]
    if not backlog_files:
        fail(f"feature backlog directory contains no program plans: {path}")
    rows = []
    row_file_programs: dict[str, str] = {}
    for backlog_file in backlog_files:
        backlog = load_json(backlog_file)
        if backlog.get("schema_version") != "glioma-feature-backlog/1.0":
            fail(f"{backlog_file} schema_version must be glioma-feature-backlog/1.0")
        file_program_id = backlog_file.stem.split("-", 1)[0]
        if path.is_dir() and backlog.get("program_id") != file_program_id:
            fail(f"{backlog_file} must declare its matching program_id")
        program_rows = backlog.get("features")
        if not isinstance(program_rows, list):
            fail(f"{backlog_file} must contain a features array")
        rows.extend(program_rows)
        for row in program_rows:
            if isinstance(row, dict) and isinstance(row.get("feature_id"), str):
                row_file_programs[row["feature_id"]] = file_program_id

    required_strings = (
        "title",
        "consumer",
        "behavior",
        "inputs",
        "outputs",
        "surface",
        "acceptance_gate",
        "prior_art_search",
    )
    seen_ids: set[str] = set()
    seen_modules: set[tuple[str, str]] = set()
    for row in rows:
        if not isinstance(row, dict):
            fail("every feature backlog row must be an object")
        feature_id = row.get("feature_id")
        if not isinstance(feature_id, str) or feature_id not in expected_ids:
            fail(f"feature backlog contains a missing, implemented, or unknown ID: {feature_id!r}")
        if feature_id in seen_ids:
            fail(f"feature backlog repeats {feature_id}")
        seen_ids.add(feature_id)
        match = re.fullmatch(r"GAF-GLIOMA-(P[0-9]{2})-F([0-9]{2})", feature_id)
        if not match:
            fail(f"feature backlog ID is malformed: {feature_id}")
        program_id = match.group(1)
        if row.get("program_id") != program_id:
            fail(f"{feature_id} is assigned to the wrong program")
        if path.is_dir() and program_id != row_file_programs.get(feature_id):
            fail(f"{feature_id} is filed under a different program's folder backlog")
        if not isinstance(row.get("module"), str) or not re.fullmatch(
            r"[a-z][a-z0-9_]*", row["module"]
        ):
            fail(f"{feature_id} must name a planned Rust module")
        module_key = (program_id, row["module"])
        if module_key in seen_modules:
            fail(f"{feature_id} collides with another planned module")
        seen_modules.add(module_key)
        module_path = (
            root
            / "crates"
            / "research"
            / "src"
            / "glioma"
            / "programs"
            / PROGRAMS[program_id]
            / f"{row['module']}.rs"
        )
        if module_path.exists():
            fail(f"{feature_id} targets an existing module without its stable feature ID: {module_path}")
        for field in required_strings:
            value = row.get(field)
            if not isinstance(value, str) or len(value.strip()) < 16:
                fail(f"{feature_id} needs a substantive {field} contract")
        if not isinstance(row.get("artifact"), str) or not re.fullmatch(
            r"[A-Z][A-Za-z0-9]{2,127}", row["artifact"]
        ):
            fail(f"{feature_id} must name a typed product artifact")
        if row.get("novelty_review") != "not_started":
            fail(f"{feature_id} novelty claims must remain explicitly unreviewed until researched")
        if row.get("research_fte_years") != 2:
            fail(f"{feature_id} must retain the minimum two-R&D-FTE-year feature scale")
        if row.get("rollout_wave") not in (1, 2, 3, 4):
            fail(f"{feature_id} rollout_wave must be an integer from 1 through 4")
        if row.get("autonomy_tier") not in ("A0", "A1", "A2", "A3", "A4"):
            fail(f"{feature_id} autonomy_tier must be A0 through A4")
        dependencies = row.get("depends_on")
        if (
            not isinstance(dependencies, list)
            or any(not isinstance(dependency, str) or dependency not in all_ids for dependency in dependencies)
            or len(dependencies) != len(set(dependencies))
            or feature_id in dependencies
        ):
            fail(f"{feature_id} dependencies must be unique known feature IDs")
        contract_text = " ".join(str(value).lower() for value in row.values())
        forbidden_patterns = (
            r"\bdiagnosis\b",
            r"\bdiagnose\b",
            r"\bclinical decision\b",
            r"\bpatient triage\b",
            r"\benroll people\b",
        )
        if any(re.search(pattern, contract_text) for pattern in forbidden_patterns):
            fail(f"{feature_id} enters an excluded clinical decision scope")

    if seen_ids != expected_ids:
        fail(
            "feature backlog must define every unimplemented slot exactly once; "
            f"missing={sorted(expected_ids - seen_ids)}, extra={sorted(seen_ids - expected_ids)}"
        )
    return sorted(rows, key=lambda row: row["feature_id"])


def markdown_cell(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ").strip()


def render_architecture(root: Path, report: dict) -> str:
    """Render a synchronized, folder-by-folder map from the plan and live Rust sources."""
    plan = (root / "docs" / "glioma" / "PROGRAM_PLAN.md").read_text(encoding="utf-8")
    lines = [
        "# Glioma research engine: folder architecture",
        "",
        "This file is generated from `PROGRAM_PLAN.md`, `organization.json`, and the live Rust "
        "module tree. It is an ownership and integration map, not evidence that every listed "
        "algorithm is biologically validated.",
        "",
        f"- Programs: {report['program_count']}",
        f"- Stable feature slots: {report['feature_slot_count']}",
        f"- Implemented feature IDs: {report['implemented_feature_count']}",
        f"- Planned feature IDs: {report['planned_feature_count']}",
        f"- Feature modules: {report['feature_module_count']}",
        f"- Workflow/composition modules: {report['workflow_composition_module_count']}",
        f"- Module-local Rust test annotations: {report['module_test_annotation_count']} "
        "(a count is not a measure of assertion quality or scientific validation)",
        "",
        "## Research workflow and ownership flow",
        "",
        "`intent → P01 evidence → P02 typed claims → P03 multimodal readiness → P04 decision "
        "context → P05 mechanisms → P06 experiments → P07 protocol simulation → P08 local "
        "instrument gateway → P09 computation → P10 interpretation/replication → P11 research "
        "object → P12 aggregate federation`",
        "",
        "A program folder owns its algorithms and public module exports. Cross-program "
        "composition remains explicit in the modules listed below; the program-level edges and "
        "release gates remain specified in `PROGRAM_PLAN.md`. Raw experimental payloads remain "
        "institution-local by default. Human-subject and clinical-source data, diagnosis, "
        "treatment, triage, and enrollment remain out of scope.",
        "",
        "## Program folders",
        "",
    ]
    for program in report["programs"]:
        program_id = program["program_id"]
        lines.extend(
            [
                f"### {program_id} — `{Path(program['source_folder']).name}`",
                "",
                "\n".join(plan_charter(plan, program_id).splitlines()[1:]),
                "",
                f"Folder inventory: {len(program['modules'])} source modules; "
                f"{program['implemented_feature_count']}/32 feature slots implemented. "
                f"The program folder directly owns {program['folder_feature_module_count']} "
                "feature modules; shared public feature facades live under "
                "`crates/research/src/glioma/`.",
                "",
                "| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |",
                "|---|---|---|---:|---|",
            ]
        )
        for module in program["modules"]:
            label = module["feature_id"] or (
                f"alias: `{module['feature_alias']}`"
                if module["feature_alias"]
                else "workflow composition"
            )
            lines.append(
                f"| [`{module['module']}`]({module['path']}) | {label} | "
                f"{markdown_cell(module['purpose'])} | {module['test_annotations']} | "
                f"{', '.join(f'`{item}`' for item in module['dependencies']) or '—'} |"
            )
        if program["shared_feature_modules"]:
            lines.extend(
                [
                    "",
                    "Shared public feature modules owned outside this folder:",
                    "",
                    "| Feature ID | Source module | Purpose | Direct test annotations |",
                    "|---|---|---|---:|",
                ]
            )
            for module in program["shared_feature_modules"]:
                lines.append(
                    f"| `{module['feature_id']}` | [`{module['module']}`]"
                    f"({module['path']}) | {markdown_cell(module['purpose'])} | "
                    f"{module['test_annotations']} |"
                )
        if program["planned_features"]:
            lines.extend(
                [
                    "",
                    "Designed implementation backlog (novelty review still pending):",
                    "",
                    "| Feature / target module | Consumer | Product behavior and contract | Acceptance gate | Effort / wave / autonomy | Depends on | Prior-art search |",
                    "|---|---|---|---|---|---|---|",
                ]
            )
            for feature in program["planned_features"]:
                contract = (
                    f"{feature['behavior']} Inputs: {feature['inputs']} "
                    f"Outputs: {feature['outputs']} Surface: {feature['surface']} "
                    f"Artifact: `{feature['artifact']}`."
                )
                lines.append(
                    f"| `{feature['feature_id']}` / `{feature['module']}.rs` | "
                    f"{markdown_cell(feature['consumer'])} | {markdown_cell(contract)} | "
                    f"{markdown_cell(feature['acceptance_gate'])} | "
                    f"{feature['research_fte_years']} research FTE-years / "
                    f"wave {feature['rollout_wave']} / {feature['autonomy_tier']} | "
                    f"{', '.join(f'`{item}`' for item in feature['depends_on']) or '—'} | "
                    f"{markdown_cell(feature['prior_art_search'])} |"
                )
        lines.extend(["", "---", ""])
    lines.extend(
        [
            "## Reading this map",
            "",
            "A feature slot is counted only when a source file declares its stable `FEATURE_ID`; "
            "unidentified modules are listed as workflow composition and do not inflate feature "
            "coverage. Shared facades are called out separately. Test-annotation counts are "
            "inventory signals only: the crate test suite, behavioral assertions, negative-path "
            "tests, replay checks, and independent scientific validation are separate gates.",
            "",
            "Regenerate and validate with `python tools/validate_glioma_organization.py "
            "--write-architecture`, then run the validator without that flag to detect drift.",
            "",
        ]
    )
    return "\n".join(lines)


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
    required_roots = (
        "engine",
        "programs",
        "contracts",
        "feature_backlog",
        "mcp",
        "organization_validator",
        "program_plan",
        "folder_architecture",
    )
    for key in required_roots:
        value = roots.get(key)
        if not isinstance(value, str) or not value.strip():
            fail(f"organization root {key!r} is missing")
    for key, value in roots.items():
        if not isinstance(key, str) or not isinstance(value, str) or not value.strip():
            fail("every organization root must map a named owner to a non-empty path")
        if not (root / value).exists():
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
    expected_program_modules = {
        path.relative_to(root).as_posix()
        for folder_name in PROGRAMS.values()
        for path in (source_root / folder_name).glob("*.rs")
        if path.name != "mod.rs"
    }
    mapped_program_modules = {
        value
        for value in roots.values()
        if value.startswith("crates/research/src/glioma/programs/")
        and value.endswith(".rs")
    }
    unmapped_modules = sorted(expected_program_modules - mapped_program_modules)
    stale_modules = sorted(mapped_program_modules - expected_program_modules)
    if unmapped_modules or stale_modules:
        fail(
            "organization.json source map and program folders differ; "
            f"unmapped={unmapped_modules}, stale={stale_modules}"
        )
    implementation_matches = [
        match.group(1)
        for path in contract_root.rglob("*.rs")
        for match in re.finditer(
            r"pub\s+const\s+FEATURE_ID:\s*&str\s*=\s*\"(GAF-GLIOMA-P\d{2}-F\d{2})\"",
            path.read_text(encoding="utf-8"),
        )
    ]
    if len(implementation_matches) != len(set(implementation_matches)):
        fail("implementation ids are duplicated")
    implemented_ids = sorted(set(implementation_matches))
    if not implemented_ids:
        fail("glioma source tree does not expose any stable implementation feature ids")

    implementation_notes = organization.get("implementation_notes", {})
    if not isinstance(implementation_notes, dict):
        fail("implementation_notes must map implemented feature ids to scope statements")
    for feature_id, note in implementation_notes.items():
        if feature_id not in implemented_ids:
            fail(f"implementation note refers to an unimplemented feature id: {feature_id}")
        if not isinstance(note, str) or not note.strip():
            fail(f"implementation note for {feature_id} is empty or invalid")

    shared_modules = shared_module_inventory(root, contract_root)
    feature_slots = {
        f"GAF-GLIOMA-{program_id}-F{slot:02}"
        for program_id in PROGRAMS
        for slot in range(1, 33)
    }
    unknown_ids = sorted(set(implemented_ids) - feature_slots)
    if unknown_ids:
        fail(f"implementation IDs do not belong to the 384-slot catalog: {unknown_ids}")
    unimplemented_ids = feature_slots - set(implemented_ids)
    planned_features = validate_feature_backlog(
        root,
        root / roots["feature_backlog"],
        unimplemented_ids,
        feature_slots,
    )

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
        modules = module_inventory(root, folder, program_id)
        folder_feature_ids = [
            module["feature_id"] for module in modules if module["feature_id"]
        ]
        if len(folder_feature_ids) != len(set(folder_feature_ids)):
            fail(f"{program_id} folder repeats a feature ID across source modules")
        feature_prefix = f"GAF-GLIOMA-{program_id}-F"
        program_ids = [feature_id for feature_id in implemented_ids if feature_id.startswith(feature_prefix)]
        shared_for_program = [
            module for module in shared_modules if module["program_id"] == program_id
        ]
        module_ids = folder_feature_ids + [
            module["feature_id"] for module in shared_for_program if module["feature_id"]
        ]
        if set(module_ids) != set(program_ids):
            fail(
                f"{program_id} source-folder/shared-module inventory does not match its "
                "stable implementation IDs"
            )
        program_unimplemented = [
            feature["feature_id"]
            for feature in planned_features
            if feature["feature_id"].startswith(feature_prefix)
        ]
        program_planned_features = [
            feature
            for feature in planned_features
            if feature["feature_id"].startswith(feature_prefix)
        ]
        folder_report.append(
            {
                "program_id": program_id,
                "source_folder": f"crates/research/src/glioma/programs/{folder_name}",
                "feature_slot_count": 32,
                "implemented_feature_count": len(program_ids),
                "implemented_feature_ids": program_ids,
                "unimplemented_feature_ids": program_unimplemented,
                "planned_features": program_planned_features,
                "folder_feature_module_count": len(folder_feature_ids),
                "modules": modules,
                "shared_feature_modules": shared_for_program,
                "workflow_composition_module_count": sum(
                    module["kind"] == "workflow_composition" for module in modules
                ),
                "module_test_annotation_count": sum(
                    module["test_annotations"] for module in modules
                ),
                "surfaces": surfaces,
                "ownership_boundary": f"{folder_name}/mod.rs",
            }
        )

    expected_total = organization["program_count"] * organization["features_per_program"]
    if expected_total != organization["feature_count"]:
        fail("organization feature cardinality is not multiplicative")
    feature_module_count = sum(
        module["kind"] == "feature"
        for program in folder_report
        for module in program["modules"]
    ) + sum(module["kind"] == "feature" for module in shared_modules)
    workflow_composition_module_count = sum(
        program["workflow_composition_module_count"] for program in folder_report
    )
    module_test_annotation_count = sum(
        program["module_test_annotation_count"] for program in folder_report
    ) + sum(module["test_annotations"] for module in shared_modules)
    return {
        "schema_version": "glioma-folder-organization/1.0",
        "program_count": len(folder_report),
        "feature_slot_count": expected_total,
        "implemented_feature_count": len(implemented_ids),
        "planned_feature_count": expected_total - len(implemented_ids),
        "feature_module_count": feature_module_count,
        "workflow_composition_module_count": workflow_composition_module_count,
        "module_test_annotation_count": module_test_annotation_count,
        "implementation_notes": dict(sorted(implementation_notes.items())),
        "planned_features": planned_features,
        "programs": folder_report,
        "shared_modules": shared_modules,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--json", action="store_true", help="emit a machine-readable report")
    parser.add_argument(
        "--write-architecture",
        action="store_true",
        help="regenerate docs/glioma/FOLDER_ARCHITECTURE.md from the checked source tree",
    )
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        report = validate(root)
        architecture_path = root / "docs" / "glioma" / "FOLDER_ARCHITECTURE.md"
        architecture = render_architecture(root, report)
        if args.write_architecture:
            architecture_path.write_text(architecture, encoding="utf-8")
        elif not architecture_path.is_file() or architecture_path.read_text(encoding="utf-8") != architecture:
            fail(
                f"folder architecture is missing or stale: {architecture_path}; "
                "run with --write-architecture"
            )
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
