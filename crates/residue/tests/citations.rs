//! The crate scanning itself for the tokens it exists not to write.
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears anywhere
//! under `crates/` or `docs/`. This crate is a register of eighty-four modules that are *not*
//! covered, and it lives under `crates/`. Written the obvious way it would take the headline to
//! 100% by itself.
//!
//! Two crates have already made a version of this mistake and both caught it the same way — by
//! scanning their own source. `crates/sweep`'s first citation test spelled the two ids it existed
//! to assert were uncited, and `crates/devplat`'s audit found that a percentage written to two
//! decimal places whose integer part falls in the section range is a citation as far as the tool is
//! concerned.
//!
//! So this file is the mechanism, not a reminder. It walks the whole crate directory — source,
//! tests and manifest — and fails on any token naming a module the register classifies.

use std::collections::BTreeSet;

use bioprism_residue::citations::{audit, crate_root};
use bioprism_residue::{audit_self, forbidden_ids, residue, scan, ModuleKey};

#[test]
fn this_crate_cites_no_module_it_says_is_uncovered() {
    let register = residue().expect("well formed");
    let report = audit_self(&register).expect("this crate's directory is readable");
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn the_scan_actually_read_the_crate_rather_than_finding_an_empty_tree() {
    // An empty scan and a clean crate produce the same verdict, and only one of them is good news.
    let register = residue().expect("well formed");
    let report = audit_self(&register).expect("readable");
    assert!(
        report.files_scanned >= 10,
        "scanned only {} files",
        report.files_scanned
    );
}

#[test]
fn the_forbidden_set_is_exactly_one_id_per_registered_module() {
    let register = residue().expect("well formed");
    let forbidden = forbidden_ids(&register);
    assert_eq!(forbidden.len(), register.len());
    for id in &forbidden {
        assert_eq!(id.len(), 5);
        assert!(id.contains('.'));
    }
}

#[test]
fn the_scanner_reproduces_the_coverage_scripts_token_rule() {
    let module = ModuleKey::new(11, 4).expect("in range");
    let sentence = format!("blueprint {} says", module.id());
    assert_eq!(scan(&sentence), BTreeSet::from([module.id()]));
}

#[test]
fn a_digit_on_either_side_stops_a_token_from_being_a_citation() {
    let id = ModuleKey::new(4, 4).expect("in range").id();
    assert!(scan(&format!("v{id}0")).is_empty());
    assert!(scan(&format!("{id}5")).is_empty());
    assert!(scan(&format!("1{id}")).is_empty());
}

#[test]
fn a_dotted_version_string_does_count_because_the_coverage_script_counts_it() {
    // `grep -E '\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b'` finds a module id inside a three-part version,
    // because a full stop is not a word character and the boundary matches. Mirroring that is
    // deliberate: teaching this audit an exception the tool lacks would make the check weaker than
    // the check it imitates, and it is the tool that moves the coverage number.
    let id = ModuleKey::new(4, 4).expect("in range").id();
    assert_eq!(scan(&format!("version 1.{id}")), BTreeSet::from([id]));
}

#[test]
fn a_two_decimal_percentage_reads_as_a_citation_and_a_one_decimal_one_does_not() {
    // `crates/devplat` nearly shipped a measurement written to two decimal places whose integer
    // part fell in the section range. The honest response is to round, not to special-case, so
    // every figure in this crate is written to one decimal place. Both strings here are assembled
    // at run time, because writing the two-decimal one down is the mistake being described.
    let two_decimals = format!("a share of {}.{} percent", 26, 19);
    let one_decimal = format!("a share of {}.{} percent", 26, 1);
    assert_eq!(scan(&two_decimals).len(), 1);
    assert!(scan(&one_decimal).is_empty());
}

#[test]
fn a_section_number_the_coverage_script_would_ignore_is_ignored_here_too() {
    let above = format!("the ratio was {}.{} to one", 50, 12);
    let below = format!("the ratio was {}.{} to one", 0, 12);
    let edge = format!("the ratio was {}.{} to one", 49, 12);
    assert!(scan(&above).is_empty());
    assert!(scan(&below).is_empty());
    assert_eq!(scan(&edge).len(), 1);
}

#[test]
fn every_registered_module_would_be_caught_if_its_id_were_written_down() {
    // The check the whole crate rests on, run against the register rather than against a fixture:
    // for each module, a text containing its id must come back forbidden.
    let register = residue().expect("well formed");
    let forbidden = forbidden_ids(&register);
    for entry in register.entries() {
        let written = format!("as recorded in {}", entry.key().id());
        let found = scan(&written);
        assert_eq!(found.len(), 1, "{}", entry.title());
        assert!(
            found.iter().all(|token| forbidden.contains(token)),
            "{} would not have been caught",
            entry.title()
        );
    }
}

#[test]
fn the_audit_refuses_a_directory_that_does_not_exist_rather_than_reporting_it_clean() {
    let register = residue().expect("well formed");
    let missing = crate_root().join("a-directory-that-is-not-there");
    assert!(audit(&missing, &register).is_err());
}

#[test]
fn any_module_shaped_token_this_crate_does_contain_names_a_module_outside_the_register() {
    // Tokens naming already-cited modules move no coverage number, so they are allowed — but they
    // are reported rather than ignored, because an id in a register of uncovered modules is worth a
    // second look even when it is harmless.
    let register = residue().expect("well formed");
    let report = audit_self(&register).expect("readable");
    for token in &report.unclassified {
        assert!(!report.forbidden.contains(token));
    }
    assert_eq!(
        report.found.len(),
        report.forbidden.len() + report.unclassified.len()
    );
}
