//! Checks over the crate's own text and shape, for rules that no type can carry.

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

#[test]
fn no_module_reads_a_clock_or_a_random_source() {
    let forbidden = [
        "SystemTime", "Instant", "rand", "thread_rng", "now()", "std::env",
    ];
    let mut offenders: Vec<String> = Vec::new();
    for (file, source) in SOURCES {
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
    }
    assert!(
        offenders.is_empty(),
        "this crate is deterministic and has no ambient inputs: {offenders:#?}"
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

    // Every §31 and §32 id the crate cites anywhere must be one of the twenty-one in scope.
    let in_scope: Vec<&str> = vec![
        "31.00", "31.01", "31.02", "31.05", "31.06", "31.07", "31.08", "31.09", "31.10", "31.11",
        "31.12", "31.13", "31.14", "31.15", "31.16", "31.17", "32.05", "32.10", "32.12", "32.13",
        "32.14", "32.15", "32.16", "32.17", "32.18", "32.19", "32.20", "32.21",
    ];
    for (file, source) in SOURCES {
        for window in source.as_bytes().windows(5) {
            let is_section = window[0] == b'3' && (window[1] == b'1' || window[1] == b'2');
            if !is_section || window[2] != b'.' {
                continue;
            }
            if !window[3].is_ascii_digit() || !window[4].is_ascii_digit() {
                continue;
            }
            let id = std::str::from_utf8(window).expect("all five bytes are ASCII");
            assert!(
                in_scope.contains(&id),
                "{file} cites {id}, which is outside this crate's scope"
            );
        }
    }
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

