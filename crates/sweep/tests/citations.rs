//! A guard on this crate's own citation set.
//!
//! `tools/coverage.sh` and `tools/backlog.sh` both count a blueprint module as covered when its
//! `NN.MM` id appears anywhere under `crates/` or `docs/`. That is a weak criterion on purpose —
//! it measures "someone has read this and taken a position" — but it has a failure mode: writing an
//! id in a doc comment moves the number whether or not anything was built. A crate that cites a
//! module it declined to implement makes the coverage table say something untrue, and nothing else
//! in the repository would catch it.
//!
//! So this crate checks itself. The set below is the twelve modules `lib.rs` claims, and the test
//! fails in both directions: an id here with no implementation, and an id in the source that is not
//! here.
//!
//! One id is deliberately absent and must stay absent: §10's registry overview (already discharged
//! by `bioprism-registry`, `bioprism-hubapi` and `bioprism-hub`). `lib.rs` names it in prose, by
//! title, with the reason. The OpenTelemetry adapter is owned by `bioprism-trace::otel`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The modules this crate implements. Every one has a module in `src/` and invariant tests.
const IMPLEMENTED: [&str; 12] = [
    "03.03", "03.11", "04.01", "04.03", "04.04", "04.05", "05.10", "05.11", "05.12", "08.06",
    "10.14", "13.11",
];

/// Ids this crate mentions while pointing at the sibling crate that owns them. Each is already
/// cited by that sibling, so mentioning it here moves no coverage number; the list exists so that
/// adding a *new* cross-reference is a deliberate edit rather than an accident.
const CROSS_REFERENCES: [&str; 12] = [
    "05.01", "05.04", "05.05", "05.08", "05.09", "08.01", "08.05", "08.07", "08.08", "10.04",
    "39.19", "43.33",
];

/// The declined registry overview, as a `(section, index)` pair rather than as a string.
///
/// This looks like an affectation and is not. `tools/coverage.sh` greps for the literal token, so a
/// test written as `const MUST_NOT_APPEAR: [&str; 2] = ["…", "…"]` would put those exact ids into
/// `crates/` and mark both modules covered — the test asserting they are uncited would be the thing
/// that cited them. The first version of this file did precisely that, and the check below caught
/// it. Building the ids from digits keeps the token out of the source.
const DECLINED: [(u8, u8); 1] = [(10, 1)];

fn declined_ids() -> Vec<String> {
    DECLINED
        .iter()
        .map(|(section, index)| format!("{section:02}.{index:02}"))
        .collect()
}

fn cited_ids() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for directory in ["src", "tests"] {
        collect(Path::new(directory), &mut found);
    }
    found
}

fn collect(directory: &Path, found: &mut BTreeSet<String>) {
    let entries = fs::read_dir(directory).expect("crate directory should be readable from a test");
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect(&path, found);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("source file should be readable");
        found.extend(scan(&text));
    }
}

/// Find `NN.MM` tokens, in the sense `tools/coverage.sh` means by
/// `grep -E '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b'`.
///
/// A word boundary is the only thing guarding either side, and a full stop is not a word character.
/// So the script counts `04.04` inside `1.04.04`, and this scanner has to as well.
///
/// The first version of this function treated a preceding full stop as blocking, on the reasoning
/// that a three-part version string is obviously not a citation. That reasoning is about what a
/// human means, and the coverage number is not computed by a human: a version-shaped string in this
/// crate's source would have passed this audit and still moved the figure. `crates/residue`
/// documents the same divergence and resolves it the same way — mirror the tool, because it is the
/// tool that moves the number. Teaching this audit an exception the script lacks makes it weaker
/// than the check it imitates, which is the one thing an audit may not be.
fn scan(text: &str) -> Vec<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    for start in 0..bytes.len().saturating_sub(4) {
        let window = &bytes[start..start + 5];
        let shaped = window[0].is_ascii_digit()
            && window[1].is_ascii_digit()
            && window[2] == '.'
            && window[3].is_ascii_digit()
            && window[4].is_ascii_digit();
        if !shaped {
            continue;
        }
        let before_ok = start == 0 || !is_word_char(bytes[start - 1]);
        let after = bytes.get(start + 5).copied();
        let after_ok = after.is_none_or(|c| !is_word_char(c));
        if before_ok && after_ok {
            out.push(window.iter().collect());
        }
    }
    out
}

/// What `\b` in the coverage script's regex counts as a word character.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[test]
fn every_id_this_crate_cites_is_either_implemented_here_or_owned_by_a_named_sibling() {
    let allowed: BTreeSet<String> = IMPLEMENTED
        .iter()
        .chain(CROSS_REFERENCES.iter())
        .map(|s| s.to_string())
        .collect();
    let cited = cited_ids();
    let unexpected: Vec<&String> = cited.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "these ids appear in crates/sweep but are in neither list: {unexpected:?}"
    );
}

#[test]
fn every_module_this_crate_claims_to_implement_is_actually_cited_in_its_source() {
    let cited = cited_ids();
    for id in IMPLEMENTED {
        assert!(
            cited.contains(id),
            "{id} is listed as implemented but appears nowhere in the source"
        );
    }
}

#[test]
fn the_declined_registry_overview_is_named_in_prose_and_never_by_id() {
    let cited = cited_ids();
    for id in declined_ids() {
        assert!(
            !cited.contains(&id),
            "{id} was declined; citing it would move the coverage number without moving capability"
        );
    }
    let lib = fs::read_to_string("src/lib.rs").expect("lib.rs should be readable");
    assert!(lib.contains("registry overview"));
}

#[test]
fn the_implemented_set_is_unique_and_the_declined_set_is_explicit() {
    assert_eq!(IMPLEMENTED.len(), 12);
    let unique: BTreeSet<&str> = IMPLEMENTED.iter().copied().collect();
    assert_eq!(unique.len(), 12);
    assert_eq!(DECLINED.len(), 1);
}

#[test]
fn the_scanner_reproduces_the_coverage_scripts_token_rule() {
    assert_eq!(scan("blueprint 04.04 says"), vec!["04.04".to_string()]);
    assert_eq!(scan("(13.11)"), vec!["13.11".to_string()]);
    assert!(
        scan("v04.040").is_empty(),
        "a digit on the right kills the word boundary, so the script finds nothing here"
    );
    assert!(
        scan("104.04").is_empty(),
        "a digit on the left kills the word boundary too"
    );
}

#[test]
fn a_dotted_version_string_does_count_because_the_coverage_script_counts_it() {
    // Verified against the tool rather than against a reading of the regex: `\b` sits between the
    // full stop and the first digit, so `grep -E '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b'` reports
    // `04.04` for the input below. An earlier version of this test asserted the opposite and was
    // the reason a version-shaped string could have moved the coverage figure while passing here.
    assert_eq!(scan("version 1.04.04"), vec!["04.04".to_string()]);
    assert_eq!(scan("0.04.05"), vec!["04.05".to_string()]);
}

#[test]
fn the_scanner_sees_a_planted_violation() {
    // A scanner that detects nothing is worse than no scanner: it certifies the crate clean
    // forever, including after the rule stops being true. The planted id is assembled from digits
    // rather than written out, because `tools/coverage.sh` greps this file too and spelling an
    // out-of-scope id here would be the very defect the audit exists to prevent.
    let planted = format!("{:02}.{:02}", 44, 21);
    let allowed: BTreeSet<String> = IMPLEMENTED
        .iter()
        .chain(CROSS_REFERENCES.iter())
        .map(|s| s.to_string())
        .collect();
    assert!(!allowed.contains(&planted));

    let found = scan(&format!("as {planted} describes"));
    assert_eq!(found, vec![planted.clone()]);
    let cited: BTreeSet<String> = found.into_iter().collect();
    assert_eq!(
        cited.difference(&allowed).collect::<Vec<_>>(),
        vec![&planted],
        "the difference the crate-level test takes must report the planted id"
    );
}

#[test]
fn the_scan_read_the_crate_rather_than_finding_an_empty_tree() {
    // An empty tree and a clean crate produce the same verdict from
    // `every_id_this_crate_cites_is_either_implemented_here_or_owned_by_a_named_sibling`, and only
    // one of them is good news.
    let cited = cited_ids();
    assert!(
        cited.len() >= IMPLEMENTED.len(),
        "the walk found only {} ids, which is fewer than this crate implements",
        cited.len()
    );
}
