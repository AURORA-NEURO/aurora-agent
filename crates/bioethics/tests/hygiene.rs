//! Checks over the crate's own text and shape, for rules no type can carry.
//!
//! Two of these are scanners, and each has a companion test that feeds it a deliberately bad value.
//! `bioprism-oraclex`'s rule applies: a scanner that detects nothing is worse than no scanner,
//! because it converts an unchecked property into a checked-looking one.

use bioprism_bioethics::action::ActionKind;
use bioprism_bioethics::dualuse::MisuseSurface;
use bioprism_bioethics::humansubject::EngagementKind;
use bioprism_bioethics::representation::ContextAxis;
use bioprism_bioethics::validation::EvidenceKind;
use bioprism_bioethics::{section_36_remainder, BlueprintModule, Impossibility};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(directory: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(directory).expect("the directory is part of this crate");
    for entry in entries {
        let path = entry.expect("a readable entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a utf-8 file name")
            .to_string();
        let body = std::fs::read_to_string(&path).expect("a readable source file");
        files.push((name, body));
    }
    files.sort();
    files
}

/// Every `NN.MM` token a coverage scan would count, with `NN` in the blueprint's section range.
fn cited_ids(source: &str) -> BTreeSet<String> {
    let bytes = source.as_bytes();
    let mut found = BTreeSet::new();
    for window in bytes.windows(5) {
        if window[2] != b'.' {
            continue;
        }
        if !window
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
        {
            continue;
        }
        let id = std::str::from_utf8(window).expect("all five bytes are ASCII");
        let section: u32 = id[..2].parse().expect("two ASCII digits");
        if !(1..=49).contains(&section) {
            continue;
        }
        found.insert(id.to_string());
    }
    found
}

fn contains_digit(text: &str) -> bool {
    text.chars().any(|character| character.is_ascii_digit())
}

/// Every string this crate records as vocabulary, as opposed to as a citation.
fn recorded_vocabulary() -> Vec<String> {
    let mut recorded: Vec<String> = section_36_remainder()
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .collect();
    recorded.extend(
        ActionKind::ALL
            .into_iter()
            .map(|kind| kind.describe().to_string()),
    );
    recorded.extend(
        MisuseSurface::ALL
            .into_iter()
            .map(|surface| surface.describe().to_string()),
    );
    recorded.extend(
        ContextAxis::ALL
            .into_iter()
            .map(|axis| axis.describe().to_string()),
    );
    recorded.extend(
        EvidenceKind::ALL
            .into_iter()
            .map(|kind| kind.describe().to_string()),
    );
    recorded.extend(
        EngagementKind::ALL
            .into_iter()
            .map(|kind| kind.describe().to_string()),
    );
    recorded.extend(
        Impossibility::ALL
            .into_iter()
            .map(|state| state.as_str().to_string()),
    );
    recorded
}

#[test]
fn no_recorded_control_name_or_vocabulary_description_contains_a_digit() {
    let recorded = recorded_vocabulary();
    assert!(
        recorded.len() >= 80,
        "only {} strings are being scanned; the vocabulary is larger than that",
        recorded.len()
    );
    for entry in &recorded {
        assert!(
            !contains_digit(entry),
            "{entry:?} contains a number, which would be this crate inventing a threshold section \
             36 does not state"
        );
    }
}

#[test]
fn the_digit_scanner_can_actually_see_one() {
    assert!(
        contains_digit("suppress any stratum with fewer than 5 members"),
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(!contains_digit("small-group protection"));
}

#[test]
fn the_two_modules_this_crate_left_out_are_never_cited_by_id() {
    let forbidden: Vec<String> = [7u32, 19u32]
        .into_iter()
        .map(|module| format!("36.{module:02}"))
        .collect();
    assert_eq!(forbidden.len(), 2);
    for id in &forbidden {
        assert!(
            format!("cites {id} in prose").contains(id.as_str()),
            "a scanner that detects nothing is worse than no scanner"
        );
    }

    let mut offenders = Vec::new();
    for directory in ["src", "tests"] {
        for (file, body) in rust_sources(&crate_root().join(directory)) {
            for id in &forbidden {
                if body.contains(id) {
                    offenders.push(format!("{directory}/{file} cites {id}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "citing a module this crate did not implement moves a coverage number without moving a \
         capability: {offenders:#?}"
    );

    for module in BlueprintModule::ALL {
        if !module.is_implemented_here() {
            assert!(module.module_id().is_none());
        }
    }
}

#[test]
fn the_citation_scanner_can_actually_see_an_id() {
    let found = cited_ids("implements blueprint 36.10 and nothing else");
    assert_eq!(
        found.len(),
        1,
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(found.contains("36.10"));
    assert!(
        cited_ids("the figure moved 78.0% and 2.3 points").is_empty(),
        "a percentage is not a citation, which is why every figure here carries one decimal"
    );
}

#[test]
fn every_blueprint_id_this_crate_cites_is_one_it_implements_or_one_a_named_sibling_owns() {
    let implemented = ["36.10", "36.11", "36.13", "36.21", "36.22"];
    let delegated = [
        "13.03", "13.04", "13.14", "13.15", "13.21", "13.22", "13.24", "13.26", "30.30", "36.01",
        "36.04", "36.06", "36.12", "36.14", "36.16", "36.17", "36.18",
    ];

    let mut offenders = Vec::new();
    for directory in ["src", "tests"] {
        for (file, body) in rust_sources(&crate_root().join(directory)) {
            for id in cited_ids(&body) {
                if implemented.contains(&id.as_str()) || delegated.contains(&id.as_str()) {
                    continue;
                }
                offenders.push(format!("{directory}/{file} cites {id}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "every id named here is either implemented here or already cited by the sibling that owns \
         it; these are neither: {offenders:#?}"
    );

    let mut cited: BTreeSet<String> = BTreeSet::new();
    for (_, body) in rust_sources(&crate_root().join("src")) {
        cited.extend(cited_ids(&body));
    }
    for id in implemented {
        assert!(
            cited.contains(id),
            "{id} is claimed as implemented and is not cited in src/"
        );
    }
}

#[test]
fn no_module_reads_a_clock_or_a_random_source() {
    let forbidden = [
        "SystemTime",
        "Instant::",
        "thread_rng",
        "rand::",
        "::now(",
        "std::env",
        "std::fs",
    ];
    let mut offenders = Vec::new();
    for (file, body) in rust_sources(&crate_root().join("src")) {
        for (number, line) in body.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for needle in forbidden {
                if line.contains(needle) {
                    offenders.push(format!("src/{file}:{}: {needle}", number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "this crate is deterministic and has no ambient inputs: {offenders:#?}"
    );
}

#[test]
fn no_source_file_declares_a_numeric_constant() {
    let mut offenders = Vec::new();
    for (file, body) in rust_sources(&crate_root().join("src")) {
        for (number, line) in body.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("").trim();
            if !code.starts_with("const ") && !code.starts_with("pub const ") {
                continue;
            }
            let Some((_, value)) = code.split_once('=') else {
                continue;
            };
            if value.chars().any(|character| character.is_ascii_digit()) {
                offenders.push(format!("src/{file}:{}: {code}", number + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "every threshold reachable from this crate is a caller-supplied parameter: {offenders:#?}"
    );
}
