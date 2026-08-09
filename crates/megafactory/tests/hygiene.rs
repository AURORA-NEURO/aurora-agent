//! Checks over the crate's own text, for rules that no type in it can carry.
//!
//! Every scanner here has a companion test that plants a violation and asserts the scanner sees it.
//! A scanner that detects nothing is worse than no scanner: it reports a clean bill of health
//! forever, including after the rule stops being true.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bioprism_megafactory::{COVERED_MODULES, SOURCES};

/// Every file under this crate's directory that `tools/coverage.sh` would read.
///
/// The scanners above run over [`SOURCES`], which is an `include_str!` list of `src/`. The coverage
/// script greps `crates/` whole, so a test file, a manifest or a `src/` subdirectory added later is
/// a place an id can be written that the baked-in list would never see. Walking the directory is
/// what closes that, and it is the same reason `crates/residue` walks rather than lists.
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

/// Constant declarations whose *value* carries a bare number.
///
/// String literals are stripped from the value before the digit test, so a table of blueprint
/// module ids or gate names is not an offender while `const MIXTURE_FLOOR: f64 = 0.15;` is. Array
/// lengths live on the declaration side and are never inspected: `[Family; 12]` says nothing about
/// biology.
fn numeric_constants(file: &str, source: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if !code.starts_with("const ") && !code.starts_with("pub const ") {
            continue;
        }
        let Some((_, value)) = code.split_once('=') else {
            continue;
        };
        if !strip_string_literals(value)
            .chars()
            .any(|character| character.is_ascii_digit())
        {
            continue;
        }
        offenders.push(format!("{file}:{}: {code}", number + 1));
    }
    offenders
}

fn strip_string_literals(value: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for character in value.chars() {
        if character == '"' {
            inside = !inside;
            continue;
        }
        if !inside {
            out.push(character);
        }
    }
    out
}

#[test]
fn the_hardcoded_constant_scanner_can_actually_see_one() {
    assert_eq!(
        numeric_constants("twin.rs", "const DECAY_RATE: f64 = 0.15;").len(),
        1,
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(
        numeric_constants(
            "lib.rs",
            r#"pub const COVERED: [&str; 2] = ["35.02", "35.03"];"#
        )
        .is_empty(),
        "a table of blueprint module ids is not a biological constant"
    );
    assert!(
        numeric_constants("boundary.rs", "pub const ALL: [BoundaryKind; 7] = [").is_empty(),
        "an array length is not a biological constant"
    );
}

#[test]
fn no_quantity_about_biology_is_hardcoded() {
    let offenders: Vec<String> = SOURCES
        .iter()
        .flat_map(|(file, source)| numeric_constants(file, source))
        .collect();
    assert!(
        offenders.is_empty(),
        "every rate, effect size, threshold and tolerance in this crate is a caller-supplied \
         parameter; these constants are not: {offenders:#?}"
    );
}

/// Lines of executable code touching a clock, the environment, or a system random source.
fn ambient_inputs(file: &str, source: &str) -> Vec<String> {
    let forbidden = [
        "SystemTime",
        "Instant::",
        "thread_rng",
        "::now(",
        "std::env",
        "rand::",
    ];
    let mut offenders = Vec::new();
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
        ambient_inputs("placement.rs", "        let taken = SystemTime::now();").len(),
        2,
        "the planted line trips both the clock needle and the now() needle"
    );
    assert!(
        ambient_inputs(
            "semisynthetic.rs",
            "    let mut rng = SplitMix64::new(seed);"
        )
        .is_empty(),
        "the workspace's seeded generator is the approved source and must not be flagged"
    );
}

#[test]
fn no_module_reads_a_clock_the_environment_or_a_system_random_source() {
    let offenders: Vec<String> = SOURCES
        .iter()
        .flat_map(|(file, source)| ambient_inputs(file, source))
        .collect();
    assert!(
        offenders.is_empty(),
        "this crate is deterministic and has no ambient inputs; timestamps are data the producer \
         supplied: {offenders:#?}"
    );
}

/// Every `NN.MM` blueprint id occurring in `source`.
///
/// Matches what `tools/coverage.sh` matches: two digits, a dot, two digits, with a word boundary on
/// each side, section 01 to 49. That is the point — a citation this scanner disagrees with is one
/// the coverage number would count and this crate would not have earned.
///
/// The boundary rule is the script's `\b`, not a reading of what a citation looks like. An earlier
/// version treated a preceding full stop as blocking, so an id sitting inside a three-part version
/// string scanned as clean while the script counted it. `crates/residue` hit the same divergence
/// and resolved it the same way: mirror the tool, because it is the tool that moves the number, and
/// an audit weaker than the check it imitates certifies nothing.
fn cited_ids(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut found = Vec::new();
    for start in 0..bytes.len().saturating_sub(4) {
        let window = &bytes[start..start + 5];
        if !window[0].is_ascii_digit()
            || !window[1].is_ascii_digit()
            || window[2] != b'.'
            || !window[3].is_ascii_digit()
            || !window[4].is_ascii_digit()
        {
            continue;
        }
        if start > 0 && is_word(bytes[start - 1]) {
            continue;
        }
        if bytes.get(start + 5).copied().is_some_and(is_word) {
            continue;
        }
        let section: u32 = std::str::from_utf8(&window[0..2])
            .expect("ascii")
            .parse()
            .expect("two digits");
        if !(1..=49).contains(&section) {
            continue;
        }
        found.push(std::str::from_utf8(window).expect("ascii").to_string());
    }
    found
}

#[test]
fn the_citation_scanner_can_actually_see_one() {
    // Assembled at run time rather than written out: an out-of-scope id spelled literally in this
    // file would be counted as a citation by `tools/coverage.sh`, which is the thing under test.
    let planted = format!("{}.{}", 35, 16);
    assert_eq!(
        cited_ids(&format!(
            "this cites {planted}, which this crate did not implement"
        )),
        vec![planted.clone()],
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(
        !COVERED_MODULES.contains(&planted.as_str()),
        "the planted id must be one this crate does not cover"
    );
    assert!(
        cited_ids("an inflation ratio of 2.857 and a share of 78.3%").is_empty(),
        "a one-decimal percentage and a three-decimal ratio are not citations"
    );
    assert!(
        cited_ids("version 0.1.0").is_empty(),
        "a single digit either side of the dot is not a module id"
    );
}

#[test]
fn a_dotted_version_string_does_count_because_the_coverage_script_counts_it() {
    // Checked against what `grep -E '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b'` actually reports rather
    // than against a reading of the regex: `\b` sits between the full stop and the first digit, so
    // the script finds a module id inside a three-part version. Both strings are assembled at run
    // time, because writing an uncovered id down here is the defect under discussion.
    let planted = format!("{}.{}", 35, 16);
    assert_eq!(
        cited_ids(&format!("version 1.{planted}")),
        vec![planted.clone()],
        "an exception this scanner has and the coverage script lacks is a hole in the audit"
    );
    assert!(
        !COVERED_MODULES.contains(&planted.as_str()),
        "the planted id must be one this crate does not cover"
    );

    let covered = format!("{}.{}", 35, 13);
    assert_eq!(cited_ids(&format!("0.{covered}")), vec![covered]);
}

#[test]
fn a_word_character_on_either_side_stops_a_token_from_being_a_citation() {
    let covered = format!("{}.{}", 35, 13);
    assert!(cited_ids(&format!("v{covered}")).is_empty());
    assert!(cited_ids(&format!("{covered}9")).is_empty());
    assert!(cited_ids(&format!("9{covered}")).is_empty());
}

#[test]
fn the_crate_cites_only_the_six_modules_it_implements() {
    for (file, source) in SOURCES {
        for id in cited_ids(source) {
            assert!(
                COVERED_MODULES.contains(&id.as_str()),
                "{file} cites {id}, which this crate did not implement; citing a module moves the \
                 coverage number without moving capability"
            );
        }
    }
}

#[test]
fn every_covered_module_is_actually_cited_somewhere() {
    for module in COVERED_MODULES {
        let mentions: usize = SOURCES
            .iter()
            .map(|(_, source)| cited_ids(source).iter().filter(|id| *id == module).count())
            .sum();
        assert!(
            mentions > 0,
            "{module} is claimed as covered but is cited in no source file"
        );
    }
}

/// Executable lines naming the count that `bioprism-scale` refuses to serialise.
fn nominal_count_uses(file: &str, source: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        if code.contains("NominalCount") || code.contains(".nominal()") {
            offenders.push(format!("{file}:{}", number + 1));
        }
    }
    offenders
}

#[test]
fn the_nominal_count_scanner_can_actually_see_one() {
    assert_eq!(
        nominal_count_uses(
            "executed.rs",
            "    let count: NominalCount = corpus.nominal();"
        )
        .len(),
        1,
        "a scanner that detects nothing is worse than no scanner"
    );
    assert!(
        nominal_count_uses("executed.rs", "//! carries no NominalCount anywhere").is_empty(),
        "prose about the guard is not a use of it"
    );
}

#[test]
fn no_executable_line_routes_around_the_effective_size_guard() {
    let offenders: Vec<String> = SOURCES
        .iter()
        .flat_map(|(file, source)| nominal_count_uses(file, source))
        .collect();
    assert!(
        offenders.is_empty(),
        "instance count is not benchmark count: a count reaches a report only inside an \
         EffectiveSize, and these lines take the other route: {offenders:#?}"
    );
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
fn no_file_anywhere_in_this_crate_cites_a_module_it_did_not_implement() {
    let files = crate_files();
    assert!(
        files.len() >= SOURCES.len(),
        "the walk found {} files, which is fewer than src/ alone holds; an empty scan and a clean \
         crate look identical from here",
        files.len()
    );
    let mut offenders = BTreeSet::new();
    for path in &files {
        let text = fs::read_to_string(path).expect("crate file is readable");
        for id in cited_ids(&text) {
            if !COVERED_MODULES.contains(&id.as_str()) {
                offenders.insert(format!("{}: {id}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the coverage script greps the whole crate directory, not just src/: {offenders:#?}"
    );
}

#[test]
fn a_fence_token_cannot_be_deserialised_into_existence() {
    let placement = SOURCES
        .iter()
        .find(|(file, _)| *file == "placement.rs")
        .map(|(_, source)| *source)
        .expect("placement.rs is in SOURCES");
    let declaration = placement
        .find("pub struct Fence(")
        .expect("placement.rs declares the fence token");
    let attributes = placement[..declaration]
        .rsplit_once("#[derive(")
        .map(|(_, derives)| derives)
        .expect("the fence token carries a derive attribute");
    assert!(
        attributes.contains("Serialize"),
        "a fence must be reportable: {attributes}"
    );
    assert!(
        !attributes.contains("Deserialize"),
        "a fence that can be deserialised from a number is a fence a worker can forge: {attributes}"
    );
}
