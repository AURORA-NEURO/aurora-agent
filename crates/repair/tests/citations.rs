//! The crate's "nothing is cited" claim, checked rather than asserted.
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its dotted token appears anywhere
//! under `crates/`, and `tools/status.sh` derives the README's Blueprint column from the same rule
//! over a crate's own sources. Neither can tell a citation from a sentence disclaiming one, so a
//! crate whose `lib.rs` says it cites nothing while writing a dotted id into the same paragraph
//! would appear in the generated table as citing a section anyway. The `classify-blueprint-modules`
//! skill records that prose discipline alone is fragile because nothing enforces it; this file is
//! the enforcement.
//!
//! **The scanner is protected from its own subject matter.** Writing a real dotted id here to test
//! the scanner would put the token under `crates/` and make the very claim false, so every id this
//! file handles is assembled from digits at run time and no literal appears in the source.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `tools/coverage.sh`'s `\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b`, hand-rolled.
///
/// Hand-rolled because this workspace builds offline against pinned versions and cannot take a
/// regex dependency, which is the same reason its CSV reader and RFC 3339 parser are hand-rolled.
/// The rule is reproduced rather than approximated: a looser scanner would pass a file the real
/// script counts, which is the only way this test could be worse than nothing.
fn blueprint_tokens(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    // `\b` treats `.` as a boundary, so a three-part version string whose last two components are
    // two digits each carries a token the real script counts — the `classify-blueprint-modules`
    // skill records exactly that accident. Excluding `.` here would make this scanner miss a
    // citation the script finds, which is the only way a clean result from it could be worse than
    // no scanner at all. (The first draft of this comment spelled such a version out, and the test
    // below failed on its own file, which is what the skill means by protecting a guard from its
    // own subject matter.)
    let boundary = |index: usize| -> bool {
        match bytes.get(index) {
            None => true,
            Some(byte) => !(byte.is_ascii_alphanumeric() || *byte == b'_'),
        }
    };
    let mut found = Vec::new();
    for dot in 2..bytes.len().saturating_sub(2) {
        if bytes[dot] != b'.' {
            continue;
        }
        let section = &bytes[dot - 2..dot];
        let module = &bytes[dot + 1..dot + 3];
        if !section.iter().all(u8::is_ascii_digit) || !module.iter().all(u8::is_ascii_digit) {
            continue;
        }
        let number = (section[0] - b'0') * 10 + (section[1] - b'0');
        if !(1..=49).contains(&number) {
            continue;
        }
        if !boundary(dot.wrapping_sub(3)) || !boundary(dot + 3) {
            continue;
        }
        found.push(String::from_utf8_lossy(&bytes[dot - 2..dot + 3]).to_string());
    }
    found
}

/// Every file the coverage script would read under this crate, in a deterministic order.
fn crate_files(from: &Path, into: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(from)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", from.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            crate_files(&entry, into);
        } else {
            into.push(entry);
        }
    }
}

#[test]
fn the_crates_own_source_writes_no_blueprint_module_id_the_coverage_script_would_count() {
    let mut files = Vec::new();
    crate_files(&crate_root(), &mut files);
    assert!(
        files.len() >= 6,
        "the walk must actually reach this crate's files, or the claim is vacuous: {files:?}"
    );

    let mut cited: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for token in blueprint_tokens(&text) {
            cited.push(format!("{}: {token}", file.display()));
        }
    }
    assert!(
        cited.is_empty(),
        "lib.rs says this crate cites nothing, and the generated tables believe the tokens rather \
         than the sentence; name the module by its title instead: {cited:?}"
    );
}

#[test]
fn the_scanner_finds_a_module_id_when_one_is_present_so_the_clean_result_means_something() {
    let section = format!("{}{}", 3, 9);
    let module = format!("{}{}", 1, 8);
    let planted = format!("the nearest neighbour is section {section}.{module}, for staleness");
    assert_eq!(
        blueprint_tokens(&planted),
        vec![format!("{section}.{module}")],
        "a scanner that cannot fire proves nothing about the file it reports clean"
    );

    // Every negative control is safe to write literally because the rule is exactly what excludes
    // it: a one-digit component, a section above 49, or section zero. A control that needed
    // escaping would be a control the scanner should have caught.
    for benign in [
        "bioprism-repair-plan/0.1",
        "the first 12 hex digits",
        "1.0.151",
        "50.01",
        "00.07",
    ] {
        assert!(
            blueprint_tokens(benign).is_empty(),
            "the rule counts sections 01 through 49 only, and a scanner that over-fires would be \
             fixed by weakening it: {benign:?} matched"
        );
    }
}
