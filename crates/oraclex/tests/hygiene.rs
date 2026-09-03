//! Checks over the crate's own text and shape, for rules that no type can carry.
//!
//! Every scanner here is paired with a test that plants a violation and asserts the scanner sees
//! it. A scanner that detects nothing is worse than no scanner: it reports a clean bill of health
//! forever, including after the rule it checks stops being true, and the passing test is what stops
//! anyone from looking. The pairing is the whole reason these checks are worth having, and the four
//! `*_can_actually_see_one` / `*_sees_a_planted_violation` tests below are that proof.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bioprism_oracle::EvidenceTier;
use bioprism_oraclex::program::Gate;
use bioprism_oraclex::standard::{ReferenceBasis, ReferenceLevel};
use bioprism_oraclex::Determination;
use bioprism_oraclex::SOURCES;

/// Constant names whose numeric contents are about the machine, not about biology.
///
/// Empty. If this list ever needs an entry, the entry is the thing to argue about in review.
const NUMERIC_CONSTANT_ALLOWLIST: [&str; 0] = [];

/// Lines in `source` that declare a constant with a numeric value.
///
/// Only the value side is inspected. `[Family; 12]` is an array length and says nothing about
/// biology; `const MIXTURE_FLOOR: f64 = 0.15;` is the thing this crate must never contain.
fn numeric_constants(file: &str, source: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if !code.starts_with("const ") && !code.starts_with("pub const ") {
            continue;
        }
        let Some((declaration, value)) = code.split_once('=') else {
            continue;
        };
        // Metadata constants are strings, even when their contract/version identifiers contain
        // digits (for example `.../1.0`). Only numeric-typed constants can encode a biological
        // threshold or default. Keep the scanner focused on those values rather than rejecting
        // harmless product identifiers.
        if declaration.contains("&str") {
            continue;
        }
        if !value.chars().any(|character| character.is_ascii_digit()) {
            continue;
        }
        if NUMERIC_CONSTANT_ALLOWLIST
            .iter()
            .any(|allowed| declaration.contains(allowed))
        {
            continue;
        }
        offenders.push(format!("{file}:{}: {code}", number + 1));
    }
    offenders
}

#[test]
fn the_hardcoded_constant_scanner_can_actually_see_one() {
    assert_eq!(
        numeric_constants("synthetic.rs", "const MIXTURE_FLOOR: f64 = 0.15;").len(),
        1,
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(
        numeric_constants("synthetic.rs", "pub const ALL: [Family; 12] = [").is_empty(),
        "an array length is not a biological constant"
    );
    assert!(
        numeric_constants(
            "synthetic.rs",
            "pub const CONTRACT_VERSION: &str = \"1.0\";"
        )
        .is_empty(),
        "a versioned string identifier is not a biological constant"
    );
}

#[test]
fn no_biological_constant_is_hardcoded() {
    let offenders: Vec<String> = SOURCES
        .iter()
        .flat_map(|(file, source)| numeric_constants(file, source))
        .collect();
    assert!(
        offenders.is_empty(),
        "every threshold in this crate is a caller-supplied parameter; these constants are not: {offenders:#?}"
    );
}

/// Lines of executable code naming a clock, the environment, or a system random source.
///
/// The needles are deliberately blunt — `rand` rather than `rand::`, `Instant` rather than
/// `Instant::` — because this crate has no legitimate use for any of them, and a blunt needle
/// over-reports rather than under-reports. `crates/megafactory` narrows the same list, because it
/// does have a seeded generator to keep clear of.
fn ambient_inputs(file: &str, source: &str) -> Vec<String> {
    let forbidden = [
        "SystemTime",
        "Instant",
        "rand",
        "thread_rng",
        "now()",
        "std::env",
    ];
    let mut offenders: Vec<String> = Vec::new();
    for (number, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for needle in forbidden {
            if line.contains(needle) {
                offenders.push(format!("{file}:{}: {needle}", number + 1));
            }
        }
    }
    offenders
}

#[test]
fn the_ambient_input_scanner_can_actually_see_one() {
    assert_eq!(
        ambient_inputs("audit.rs", "        let taken = SystemTime::now();").len(),
        2,
        "the planted line trips both the clock needle and the now() needle"
    );
    assert_eq!(
        ambient_inputs("panel.rs", "    let mut rng = thread_rng();").len(),
        1
    );
    assert_eq!(
        ambient_inputs("panel.rs", "use rand::rngs::StdRng;").len(),
        1
    );
    assert_eq!(
        ambient_inputs("program.rs", "    let root = std::env::var(\"ROOT\");").len(),
        1
    );
    assert!(
        ambient_inputs("units.rs", "// no module reads a clock or a random source").is_empty(),
        "prose about the rule is not a breach of it"
    );
}

#[test]
fn no_module_reads_a_clock_or_a_random_source() {
    let offenders: Vec<String> = SOURCES
        .iter()
        .flat_map(|(file, source)| ambient_inputs(file, source))
        .collect();
    assert!(
        offenders.is_empty(),
        "this crate is deterministic and has no ambient inputs: {offenders:#?}"
    );
}

/// The §31 and §32 ids this crate is entitled to have anywhere in its text.
///
/// `tools/coverage.sh` counts a module as covered when its id appears anywhere under `crates/`, so
/// an id written here is a claim about capability. Every entry is either a module this crate checks
/// something for or one named in the delegation paragraph and already owned by `bioprism-oracle`.
const IN_SCOPE: [&str; 28] = [
    "31.00", "31.01", "31.02", "31.05", "31.06", "31.07", "31.08", "31.09", "31.10", "31.11",
    "31.12", "31.13", "31.14", "31.15", "31.16", "31.17", "32.05", "32.10", "32.12", "32.13",
    "32.14", "32.15", "32.16", "32.17", "32.18", "32.19", "32.20", "32.21",
];

/// Every `31.NN` or `32.NN` token in `source`.
///
/// No boundary test on either side, which makes this stricter than the coverage script rather than
/// laxer: a token the script would skip for abutting a digit is still reported here. Strictness in
/// that direction is free — the worst case is an argument in review — whereas an exception the
/// script lacks is a citation that moves the number while passing the audit, which is what
/// `crates/sweep` and `crates/megafactory` both got wrong in their own scanners.
fn cited_section_ids(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for window in source.as_bytes().windows(5) {
        let is_section = window[0] == b'3' && (window[1] == b'1' || window[1] == b'2');
        if !is_section || window[2] != b'.' {
            continue;
        }
        if !window[3].is_ascii_digit() || !window[4].is_ascii_digit() {
            continue;
        }
        found.push(
            std::str::from_utf8(window)
                .expect("all five bytes are ASCII")
                .to_string(),
        );
    }
    found
}

#[test]
fn the_citation_scanner_sees_a_planted_violation() {
    // Assembled from digits rather than written out: the coverage script greps this file too, so an
    // out-of-scope id spelled literally here would be the citation the audit exists to catch.
    let planted = format!("{}.{}", 32, 40);
    assert!(!IN_SCOPE.contains(&planted.as_str()));
    assert_eq!(
        cited_section_ids(&format!("as {planted} requires")),
        vec![planted],
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(
        cited_section_ids("a ratio of 31.7 and a share of 4.32 percent").is_empty(),
        "one digit after the dot is not a module id"
    );
    assert!(
        cited_section_ids("section 33 and section 30").is_empty(),
        "only the two sections in scope are scanned for"
    );
}

#[test]
fn the_crate_cites_only_modules_it_checks_something_for() {
    let lib = SOURCES
        .iter()
        .find(|(file, _)| *file == "lib.rs")
        .map(|(_, source)| *source)
        .expect("lib.rs is in SOURCES");

    // Ids named in prose as belonging to a sibling must not be cited as coverage.
    for delegated in ["31.13", "31.16"] {
        let mentions = lib.matches(delegated).count();
        assert!(
            mentions > 0,
            "{delegated} should be named in the delegation paragraph"
        );
    }

    let mut seen = BTreeSet::new();
    for (file, source) in SOURCES {
        for id in cited_section_ids(source) {
            assert!(
                IN_SCOPE.contains(&id.as_str()),
                "{file} cites {id}, which is outside this crate's scope"
            );
            seen.insert(id);
        }
    }
    assert!(
        seen.len() > IN_SCOPE.len() / 2,
        "the scan found only {} of the {} ids in scope, which is what an empty read looks like",
        seen.len(),
        IN_SCOPE.len()
    );
}

/// Every file under this crate's directory that `tools/coverage.sh` would read.
fn crate_files() -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(Path::new(env!("CARGO_MANIFEST_DIR")), &mut found);
    found.sort();
    found
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("the crate directory is readable from a test") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            walk(&path, found);
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str());
        if matches!(extension, Some("rs") | Some("toml") | Some("md")) {
            found.push(path);
        }
    }
}

#[test]
fn no_file_anywhere_in_this_crate_cites_a_module_outside_its_scope() {
    // The scanner above reads SOURCES, which is `src/`. The coverage script greps the crate whole,
    // so a test file or a manifest is somewhere an id can be written that SOURCES would never see.
    let files = crate_files();
    assert!(
        files.len() > SOURCES.len(),
        "the walk found {} files, fewer than src/ alone holds",
        files.len()
    );
    let mut offenders = BTreeSet::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("crate file is readable");
        for id in cited_section_ids(&text) {
            if !IN_SCOPE.contains(&id.as_str()) {
                offenders.insert(format!("{}: {id}", path.display()));
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

#[test]
fn the_baked_in_source_list_names_every_file_under_src() {
    // Every scanner in this file reads SOURCES. A module added to `src/` and not added to the list
    // is invisible to all of them, and the suite goes on reporting a clean crate — the failure the
    // scanners exist to prevent, one level up.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut on_disk = Vec::new();
    walk(&src, &mut on_disk);
    let on_disk: BTreeSet<String> = on_disk
        .iter()
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("rs"))
        .map(|path| {
            path.strip_prefix(&src)
                .expect("under src")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let listed: BTreeSet<String> = SOURCES.iter().map(|(file, _)| file.to_string()).collect();
    assert_eq!(
        on_disk, listed,
        "SOURCES and src/ disagree; a file missing from the list is a file no hygiene check reads"
    );
}

#[test]
fn every_release_gate_states_the_sentence_it_enforces() {
    assert_eq!(Gate::ALL.len(), 8, "§32 states eight release gates");
    for gate in Gate::ALL {
        assert!(gate.as_str().len() > 20, "{gate:?} has no sentence");
    }
}

#[test]
fn no_reference_basis_claims_the_deterministic_rung_from_a_measurement_process() {
    let processes = [
        ReferenceBasis::ReaderConsensus {
            rule: "majority".into(),
            readers: 3,
        },
        ReferenceBasis::OrthogonalAssay {
            assay: "immunohistochemistry".into(),
        },
        ReferenceBasis::IntegratedClassifier {
            classifier: "methylation".into(),
            classifier_version: "1".into(),
            ontology_version: "2024".into(),
        },
        ReferenceBasis::Simulator {
            model: "growth".into(),
            known_misspecification: "no immune compartment".into(),
        },
    ];
    for basis in processes {
        assert!(
            basis.ceiling() < EvidenceTier::Deterministic,
            "{} is a measurement process and cannot be a proof of defect",
            basis.kind()
        );
    }
}

#[test]
fn a_geometric_reference_distinguishes_five_levels() {
    assert_eq!(ReferenceLevel::ALL.len(), 5);
    let names: Vec<&str> = ReferenceLevel::ALL.iter().map(|l| l.as_str()).collect();
    assert!(names.contains(&"detection"));
    assert!(names.contains(&"downstream_use"));
}

#[test]
fn a_determination_serialises_the_same_way_every_time() {
    let determination = Determination::unresolved("a second reagent", "one reagent agreed");
    let first = serde_json::to_string(&determination).expect("serialisable");
    let second = serde_json::to_string(&determination).expect("serialisable");
    assert_eq!(first, second);
    assert!(first.starts_with(r#"{"determination":"unresolved""#));
}
