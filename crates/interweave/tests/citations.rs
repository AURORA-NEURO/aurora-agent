//! The mechanism behind a sentence `lib.rs` had been stating as a fact.
//!
//! [`IMPLEMENTED_MODULES`] is documented with the claim that "no id appears anywhere in this crate
//! that is not in this list or already owned by a sibling, which is the discipline that keeps a
//! coverage percentage from moving without capability moving with it". Until this file existed the
//! constant was referenced nowhere and the crate had no citation test, so the claim was true only
//! for as long as nobody wrote another id — in the crate with the largest citation footprint in the
//! workspace, forty-one distinct §23 ids for nine implemented modules.
//!
//! `crates/oraclex` names the failure this avoids: a scanner that detects nothing is worse than no
//! scanner, because a passing test is what stops anyone from looking again. A stated rule with no
//! scanner at all is one rung below that, and this is the rung it comes up to.
//!
//! # What "owned by a sibling" has to mean
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` id appears anywhere
//! under `crates/` or `docs/`, with `docs/BACKLOG.md` — the register of what is *not* covered —
//! excluded. So an id written here moves the coverage figure exactly when this crate is the only
//! place it appears. That, and not a hand-maintained allowlist, is the property worth checking: the
//! audit re-derives ownership from the tree the script reads, so a sibling dropping its last
//! citation of a module turns this crate's mention into a coverage-moving one and fails here.
//!
//! The token rule below reproduces the script's regex rather than shelling out to it, for the
//! reason `crates/residue` gives: a test that invokes the script tests the script, and a test that
//! reimplements its rule fails when the two drift, which is the event worth catching.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use bioprism_interweave::IMPLEMENTED_MODULES;

/// Every blueprint-module-shaped token in `text`, in the coverage script's sense.
///
/// `\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b` spelled out: two digits naming a section from 01 to 49, a
/// full stop, two digits, and a word boundary on each side. A full stop is not a word character, so
/// an id inside a three-part version string counts — `crates/sweep` and `crates/megafactory` both
/// exempted that case and were laxer than the tool they mirrored as a result.
fn scan(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut found = BTreeSet::new();
    for start in 0..bytes.len().saturating_sub(4) {
        if start > 0 && is_word(bytes[start - 1]) {
            continue;
        }
        let window = &bytes[start..start + 5];
        if !window[0].is_ascii_digit()
            || !window[1].is_ascii_digit()
            || window[2] != b'.'
            || !window[3].is_ascii_digit()
            || !window[4].is_ascii_digit()
        {
            continue;
        }
        if bytes.get(start + 5).copied().is_some_and(is_word) {
            continue;
        }
        let section = (window[0] - b'0') * 10 + (window[1] - b'0');
        if !(1..=49).contains(&section) {
            continue;
        }
        found.insert(String::from_utf8_lossy(window).into_owned());
    }
    found
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

/// Files a tree holds that `tools/coverage.sh` would read.
///
/// `docs/BACKLOG.md` is skipped because the script skips it: it enumerates the *uncovered* ids, so
/// counting it as ownership would let the backlog vouch for every citation in the workspace.
fn readable_files(root: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("directory is readable") {
        let path = entry.expect("directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if path.is_dir() {
            if name != "target" {
                readable_files(&path, found);
            }
            continue;
        }
        if name == "BACKLOG.md" {
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str());
        if matches!(extension, Some("rs") | Some("toml") | Some("md")) {
            found.push(path);
        }
    }
}

fn read(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).expect("file is readable")).into_owned()
}

/// Which of this crate's files cite which ids.
fn cited_here() -> BTreeMap<String, BTreeSet<String>> {
    let root = crate_root();
    let mut files = Vec::new();
    readable_files(&root, &mut files);
    assert!(
        files.len() >= 10,
        "the walk found {} files under {}; an empty walk and a clean crate look identical from \
         here, and only one of them is good news",
        files.len(),
        root.display()
    );
    let mut by_id: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in files {
        let where_from = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for id in scan(&read(&path)) {
            by_id.entry(id).or_default().insert(where_from.clone());
        }
    }
    by_id
}

/// Every id cited anywhere the coverage script looks, except inside this crate.
fn cited_by_a_sibling() -> BTreeSet<String> {
    let repo = repo_root();
    let mine = crate_root();
    let mut files = Vec::new();
    readable_files(&repo.join("crates"), &mut files);
    readable_files(&repo.join("docs"), &mut files);
    let elsewhere: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| !path.starts_with(&mine))
        .collect();
    assert!(
        elsewhere.len() >= 500,
        "the workspace walk found only {} files outside this crate; a short walk would vouch for \
         nothing and report it as ownership",
        elsewhere.len()
    );
    let mut found = BTreeSet::new();
    for path in &elsewhere {
        found.extend(scan(&read(path)));
    }
    found
}

#[test]
fn the_scanner_reproduces_the_coverage_scripts_token_rule() {
    let id = format!("{:02}.{:02}", 23, 24);
    assert_eq!(scan(&format!("blueprint {id} says")), BTreeSet::from([id.clone()]));
    assert_eq!(scan(&format!("({id})")), BTreeSet::from([id.clone()]));
    assert!(
        scan(&format!("9{id}")).is_empty(),
        "a digit on the left kills the word boundary"
    );
    assert!(
        scan(&format!("{id}9")).is_empty(),
        "and a digit on the right kills it too"
    );
    assert!(
        scan(&format!("v{id}")).is_empty(),
        "as does a letter, which is what \\b means"
    );
    assert!(
        scan(&format!("the ratio was {}.{} to one", 50, 12)).is_empty(),
        "section 50 is outside the script's alternation"
    );
    assert!(
        scan("a share of 78.3% and a ratio of 2.857").is_empty(),
        "a one-decimal percentage is not a citation"
    );
}

#[test]
fn a_dotted_version_string_does_count_because_the_coverage_script_counts_it() {
    // The exemption `crates/sweep` and `crates/megafactory` both granted, and the reason a
    // version-shaped string in either could have moved the figure while passing its own audit. A
    // full stop is not a word character, so `\b` matches after it and the script reports the id.
    let id = format!("{:02}.{:02}", 23, 24);
    assert_eq!(scan(&format!("version 1.{id}")), BTreeSet::from([id.clone()]));
    assert_eq!(scan(&format!("0.{id}")), BTreeSet::from([id]));
}

/// The verdict: ids this crate writes that neither its list nor the rest of the tree accounts for.
///
/// Separated from the walk so a planted violation can be put through the same decision procedure
/// the real audit uses, rather than through a paraphrase of it.
fn unaccounted_for(
    here: &BTreeMap<String, BTreeSet<String>>,
    owned_elsewhere: &BTreeSet<String>,
) -> Vec<String> {
    here.iter()
        .filter(|(id, _)| {
            !IMPLEMENTED_MODULES.contains(&id.as_str()) && !owned_elsewhere.contains(*id)
        })
        .map(|(id, files)| {
            format!(
                "{id} appears in {} and nowhere else the coverage script reads",
                files.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .collect()
}

/// A module-shaped token that appears nowhere the coverage script looks.
///
/// Found by search rather than written down, for two reasons. Writing one down would put it under
/// `crates/` and stop it being absent — the mistake `crates/sweep`'s first citation test made. And
/// every one of section 23's own modules is cited somewhere in this workspace, so there is no
/// hand-picked §23 id that would serve.
fn an_id_no_file_in_the_workspace_contains(owned_elsewhere: &BTreeSet<String>) -> String {
    for section in 1..=49u8 {
        for index in 0..=99u8 {
            let candidate = format!("{section:02}.{index:02}");
            if !owned_elsewhere.contains(&candidate)
                && !IMPLEMENTED_MODULES.contains(&candidate.as_str())
            {
                return candidate;
            }
        }
    }
    panic!(
        "every one of the 4900 possible ids is already cited somewhere, which would mean the \
         ownership set vouches for anything and this audit checks nothing"
    );
}

#[test]
fn the_audit_sees_a_planted_violation() {
    // A scanner that detects nothing is worse than no scanner. The planted id goes through the same
    // `unaccounted_for` the real test calls, against the real ownership set, so what is proved here
    // is the procedure that certifies the crate and not a restatement of it.
    let owned_elsewhere = cited_by_a_sibling();
    let planted = an_id_no_file_in_the_workspace_contains(&owned_elsewhere);
    assert_eq!(
        scan(&format!("this crate also handles {planted}")),
        BTreeSet::from([planted.clone()]),
        "the token rule must see the planted id before anything else can"
    );

    let here = BTreeMap::from([(planted.clone(), BTreeSet::from(["src/lib.rs".to_string()]))]);
    let offenders = unaccounted_for(&here, &owned_elsewhere);
    assert_eq!(offenders.len(), 1, "{offenders:?}");
    assert!(offenders[0].contains(&planted));
    assert!(offenders[0].contains("src/lib.rs"));

    let implemented = BTreeMap::from([(
        IMPLEMENTED_MODULES[0].to_string(),
        BTreeSet::from(["src/adapter.rs".to_string()]),
    )]);
    assert!(
        unaccounted_for(&implemented, &owned_elsewhere).is_empty(),
        "an implemented module is not a violation"
    );
}

#[test]
fn the_ownership_set_is_a_real_reading_of_the_tree_rather_than_a_rubber_stamp() {
    // `unaccounted_for` waves through anything the ownership set contains, so an over-broad set
    // would make the audit vacuous while it kept passing. Two floors: it holds the ids §23's
    // siblings actually own, and it does not hold everything.
    let owned_elsewhere = cited_by_a_sibling();
    assert!(
        owned_elsewhere.len() > 100,
        "only {} ids found across the workspace",
        owned_elsewhere.len()
    );
    for index in [5u8, 16, 46] {
        let id = format!("{:02}.{:02}", 23, index);
        assert!(
            owned_elsewhere.contains(&id),
            "{id} is a sibling's and the walk did not find it"
        );
    }
    let absent = an_id_no_file_in_the_workspace_contains(&owned_elsewhere);
    assert!(!owned_elsewhere.contains(&absent));
}

#[test]
fn no_id_this_crate_writes_is_one_it_neither_implements_nor_finds_already_owned() {
    let offenders = unaccounted_for(&cited_here(), &cited_by_a_sibling());
    assert!(
        offenders.is_empty(),
        "each of these moves the coverage figure on this crate's word alone: {offenders:#?}"
    );
}

#[test]
fn every_module_this_crate_claims_is_cited_in_the_module_that_implements_it() {
    // The other half of the claim: the list is not merely a fence around what may be written, it is
    // supposed to be backed by code. An id cited only in lib.rs's table would be a coverage number
    // resting on a table of contents.
    let table = [
        ("adapter.rs", 24),
        ("component.rs", 25),
        ("microbench.rs", 27),
        ("credit.rs", 28),
        ("threat.rs", 29),
        ("conformance.rs", 31),
        ("workflow.rs", 33),
        ("packs.rs", 39),
        ("human.rs", 47),
    ];
    assert_eq!(table.len(), IMPLEMENTED_MODULES.len());
    let src = crate_root().join("src");
    for (file, index) in table {
        let id = format!("{:02}.{:02}", 23, index);
        assert!(
            IMPLEMENTED_MODULES.contains(&id.as_str()),
            "{id} is in the table above but not in IMPLEMENTED_MODULES"
        );
        assert!(
            scan(&read(&src.join(file))).contains(&id),
            "{id} is claimed as implemented and {file} does not cite it"
        );
    }
}

#[test]
fn the_declared_list_is_nine_distinct_ids_all_in_section_23() {
    let unique: BTreeSet<&str> = IMPLEMENTED_MODULES.iter().copied().collect();
    assert_eq!(unique.len(), IMPLEMENTED_MODULES.len());
    for id in IMPLEMENTED_MODULES {
        assert_eq!(scan(id), BTreeSet::from([id.to_string()]));
        assert!(id.starts_with("23."));
    }
}

#[test]
fn the_crate_cites_more_ids_than_it_implements_which_is_why_the_audit_is_needed() {
    // Recorded rather than implied. This crate names forty-one distinct §23 ids for nine
    // implemented modules, the widest gap in the workspace, and every one of the other thirty-two
    // is prose about who owns what. That ratio is exactly what makes an unenforced ownership claim
    // expensive here.
    let here = cited_here();
    assert!(
        here.len() > IMPLEMENTED_MODULES.len() * 3,
        "found {} distinct ids, which is not the footprint this audit was written for",
        here.len()
    );
}
