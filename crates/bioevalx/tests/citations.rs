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

/// Every `.rs` file under `src` and `tests`, at any depth.
///
/// Walked rather than listed one level deep. The coverage script greps `crates/` whole, so a file
/// in a subdirectory is a place an id can be written; a one-level reader would report a clean
/// citation set for a tree it never opened, and say nothing about having stopped looking.
fn crate_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for directory in ["src", "tests"] {
        collect_rust_files(&root.join(directory), &mut out);
    }
    out.sort();
    out
}

fn collect_rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, found);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            found.push(path);
        }
    }
}

/// Every `NN.MM` token, matching what `tools/coverage.sh` greps for.
///
/// A word boundary on each side and a section between 01 and 49, which is
/// `\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b` spelled out. A full stop is not a word character, so an id
/// inside a three-part version string counts here for the same reason it counts there.
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
        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let before_ok = start == 0 || !is_word(bytes[start - 1]);
        let after_ok = start + 5 == bytes.len() || !is_word(bytes[start + 5]);
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
fn the_citation_scanner_sees_a_planted_violation() {
    // `crates/oraclex` states the principle these companion tests exist for: a scanner that detects
    // nothing is worse than no scanner, because it certifies a clean crate forever, including after
    // the rule stops being true. Every id below is assembled from digits rather than written out —
    // `tools/coverage.sh` greps this file too, so spelling an out-of-scope id here would itself be
    // the citation the audit exists to catch.
    let planted = format!("{:02}.{:02}", 44, 21);
    let allowed: BTreeSet<String> = IMPLEMENTED_MODULES
        .iter()
        .chain(CITED_BUT_OWNED_ELSEWHERE.iter())
        .map(|s| s.to_string())
        .collect();
    assert!(!allowed.contains(&planted));
    assert_eq!(
        module_ids(&format!("as {planted} requires")),
        BTreeSet::from([planted.clone()])
    );

    let in_scope = format!("{:02}.{:02}", 26, 17);
    assert_eq!(
        module_ids(&format!("version 1.{in_scope}")),
        BTreeSet::from([in_scope.clone()]),
        "the coverage script's word boundary matches after a full stop, so a version-shaped string \
         is a citation as far as the number is concerned"
    );
    assert!(
        module_ids(&format!("v{in_scope}")).is_empty(),
        "a letter on the left kills the word boundary, and the script finds nothing there either"
    );
    assert!(
        module_ids(&format!("{in_scope}9")).is_empty(),
        "nor with a digit on the right"
    );
    assert!(
        module_ids("a share of 78.3% and a ratio of 2.857").is_empty(),
        "a one-decimal percentage is not a citation"
    );
    assert!(
        module_ids(&format!("the ratio was {}.{} to one", 50, 12)).is_empty(),
        "section 50 is outside the blueprint and outside the script's alternation"
    );
}

#[test]
fn the_walk_reads_the_crate_rather_than_finding_an_empty_tree() {
    // Every citation test below reports the same verdict for a clean crate and for a walk that
    // opened nothing, and only one of those is good news.
    let files = crate_files();
    assert!(
        files.len() >= 10,
        "the walk found only {} files",
        files.len()
    );
    let total: usize = files
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).expect("source is readable");
            module_ids(&text).len()
        })
        .sum();
    assert!(total > 0, "the scanner found no module id anywhere");
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
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);
    let mut cited_outside_lib = BTreeSet::new();
    for path in files {
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
