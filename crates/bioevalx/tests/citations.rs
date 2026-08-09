//! The citation set is checked, not asserted.
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` id appears anywhere
//! under `crates/` or `docs/`. That makes a citation a claim about capability, and an unbacked one
//! moves the coverage number without moving anything real. These tests read this crate's own source
//! and hold the claim to the implementation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bioprism_bioevalx::{BIOEVALX_SCHEMA_VERSION, CITED_BUT_OWNED_ELSEWHERE, IMPLEMENTED_MODULES};

fn crate_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for directory in ["src", "tests"] {
        for entry in fs::read_dir(root.join(directory)).expect("directory is readable") {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

/// Every `NN.MM` token, matching what `tools/coverage.sh` greps for.
fn module_ids(text: &str) -> BTreeSet<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut found = BTreeSet::new();
    for start in 0..bytes.len() {
        if start + 5 > bytes.len() {
            break;
        }
        let window: String = bytes[start..start + 5].iter().collect();
        let digits: Vec<char> = window.chars().collect();
        let shaped = digits[0].is_ascii_digit()
            && digits[1].is_ascii_digit()
            && digits[2] == '.'
            && digits[3].is_ascii_digit()
            && digits[4].is_ascii_digit();
        if !shaped {
            continue;
        }
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = start + 5 == bytes.len() || !bytes[start + 5].is_ascii_alphanumeric();
        if !before_ok || !after_ok {
            continue;
        }
        let section: u32 = window[..2].parse().expect("two ascii digits");
        if (1..=49).contains(&section) {
            found.insert(window);
        }
    }
    found
}

#[test]
fn the_crate_cites_exactly_the_modules_it_implements() {
    let mut found = BTreeSet::new();
    for path in crate_files() {
        let text = fs::read_to_string(&path).expect("source is readable");
        for id in module_ids(&text) {
            if id.starts_with("26.") || id.starts_with("07.") {
                found.insert(id);
            }
        }
    }
    let allowed: BTreeSet<String> = IMPLEMENTED_MODULES
        .iter()
        .chain(CITED_BUT_OWNED_ELSEWHERE.iter())
        .map(|s| s.to_string())
        .collect();

    let unbacked: Vec<&String> = found.difference(&allowed).collect();
    assert!(
        unbacked.is_empty(),
        "these ids are cited without being implemented here: {unbacked:?}"
    );

    let implemented: BTreeSet<String> = IMPLEMENTED_MODULES.iter().map(|s| s.to_string()).collect();
    let missing: Vec<&String> = implemented.difference(&found).collect();
    assert!(
        missing.is_empty(),
        "these ids are claimed implemented but never cited in the source: {missing:?}"
    );
}

#[test]
fn the_biocapability_atlas_module_is_never_cited_because_siblings_own_it() {
    // Assembled rather than written out: `tools/coverage.sh` greps this file too, so the literal
    // id appearing here would itself mark the module covered — which is the exact defect the test
    // exists to prevent.
    let atlas_module = format!("{}.{}", 26, 19);
    for path in crate_files() {
        let text = fs::read_to_string(&path).expect("source is readable");
        assert!(
            !module_ids(&text).contains(&atlas_module),
            "{} cites the atlas module that bioprism-atlas, bioprism-metrics and \
             bioprism-evalengine already discharge",
            path.display()
        );
    }
}

#[test]
fn no_module_id_outside_the_two_sections_in_scope_is_cited() {
    let mut strays = BTreeSet::new();
    for path in crate_files() {
        let text = fs::read_to_string(&path).expect("source is readable");
        for id in module_ids(&text) {
            if !id.starts_with("26.") && !id.starts_with("07.") {
                strays.insert(id);
            }
        }
    }
    assert!(
        strays.is_empty(),
        "referring to another section by id would move its coverage number; name the crate that \
         owns it instead. Found: {strays:?}"
    );
}

#[test]
fn every_implemented_module_has_a_source_module_that_cites_it() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut cited_outside_lib = BTreeSet::new();
    for entry in fs::read_dir(&src).expect("src is readable") {
        let path = entry.expect("directory entry").path();
        if path.file_name().and_then(|n| n.to_str()) == Some("lib.rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("source is readable");
        for id in module_ids(&text) {
            cited_outside_lib.insert(id);
        }
    }
    for id in IMPLEMENTED_MODULES {
        assert!(
            cited_outside_lib.contains(id),
            "{id} is cited only in lib.rs; an implemented module is cited where it is implemented"
        );
    }
}

#[test]
fn the_schema_version_names_this_crate() {
    assert!(BIOEVALX_SCHEMA_VERSION.starts_with("bioprism-bioevalx/"));
}
