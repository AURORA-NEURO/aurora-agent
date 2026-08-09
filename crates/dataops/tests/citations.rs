//! The coverage script counts a module as covered when its id appears anywhere under `crates/`.
//! This asserts that the ids appearing in this crate are exactly the seven it implements.
//!
//! Reading files is the one place this crate touches a filesystem, and it is a test rather than
//! the library: `citations::scan` takes text, and the bytes are supplied here.

use bioprism_dataops::citations::{declared, scan, DECLARED};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn crate_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).expect("the crate directory is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_crate_cites_exactly_the_seven_modules_it_implements() {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut scanned = 0usize;
    for path in crate_files() {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        scanned += 1;
        found.extend(scan(&text));
    }

    assert!(scanned >= 12, "expected to scan the whole crate, saw {scanned} files");
    assert_eq!(found, declared());
}

/// The positive cases use ids this crate already declares, on purpose: writing an in-range id
/// this crate does not implement would move the coverage figure from inside its own audit test.
#[test]
fn the_scanner_reproduces_the_coverage_scripts_token_rule() {
    assert_eq!(scan("implements 12.02 and 12.16"), {
        let mut expected = BTreeSet::new();
        expected.insert("12.02".to_string());
        expected.insert("12.16".to_string());
        expected
    });
    assert!(scan("version 0.1.0").is_empty());
    assert!(scan("99.95 percent").is_empty());
    assert!(scan("50.01 is above the section range").is_empty());
    assert!(scan("00.12 is below the section range").is_empty());
    assert!(scan("a12.02 has no leading word boundary").is_empty());
    assert!(scan("12.021 has no trailing word boundary").is_empty());
    assert!(scan("69.8% is written to one decimal").is_empty());
}

#[test]
fn the_declared_set_is_the_seven_section_twelve_modules_in_scope() {
    assert_eq!(DECLARED.len(), 7);
    assert!(DECLARED.iter().all(|id| id.starts_with("12.")));
}
