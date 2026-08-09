//! Checks over the crate's own text, for rules that no type in it can carry.
//!
//! Every scanner here has a companion test that plants a violation and asserts the scanner sees it.
//! A scanner that detects nothing is worse than no scanner: it reports a clean bill of health
//! forever, including after the rule stops being true.

use bioprism_megafactory::{COVERED_MODULES, SOURCES};

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
/// Matches what `tools/coverage.sh` matches: two digits, a dot, two digits, not adjacent to another
/// digit. That is the point — a citation this scanner disagrees with is one the coverage number
/// would count and this crate would not have earned.
fn cited_ids(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
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
        if start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
            continue;
        }
        if bytes.get(start + 5).is_some_and(u8::is_ascii_digit) {
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
        "a semantic version is not a citation"
    );
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
