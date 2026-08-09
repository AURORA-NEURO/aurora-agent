//! The register against the live backlog, and against the crates it quotes.
//!
//! Drift between `docs/BACKLOG.md` and this register is the thing the crate exists to catch, so it
//! is a test rather than a report nobody runs. Four crates are being written against these sections
//! right now; when one of them cites a module, `tools/backlog.sh` drops the line, and the test
//! below turns red until the entry is deleted. That is the intended workflow, not a failure of it.

use bioprism_cookbook::Workspace;
use bioprism_residue::reconcile::{parse_backlog, reconcile_in, SourceProblem, BACKLOG_PATH};
use bioprism_residue::{reconcile, residue, Register};

fn register() -> Register {
    residue().expect("the shipped register is well formed")
}

#[test]
fn every_module_the_backlog_lists_is_explained_here_and_every_entry_here_is_still_listed() {
    let report = reconcile(&register()).expect("the workspace is readable");
    assert!(
        report.unexplained.is_empty() && report.stale.is_empty(),
        "{}",
        report.render()
    );
}

#[test]
fn the_register_holds_exactly_as_many_modules_as_the_backlog() {
    let report = reconcile(&register()).expect("the workspace is readable");
    assert_eq!(report.register_entries, report.backlog_entries);
}

#[test]
fn every_module_carries_the_title_the_backlog_gives_it() {
    let report = reconcile(&register()).expect("the workspace is readable");
    assert!(report.retitled.is_empty(), "{}", report.render());
}

#[test]
fn every_crate_named_in_a_verdict_is_a_workspace_member() {
    let report = reconcile(&register()).expect("the workspace is readable");
    assert!(
        report.unknown_crates.is_empty(),
        "these crates are named and do not exist: {:?}",
        report.unknown_crates
    );
}

#[test]
fn every_judgement_still_says_what_this_register_attributes_to_it() {
    let report = reconcile(&register()).expect("the workspace is readable");
    let reworded: Vec<_> = report
        .source_defects
        .iter()
        .filter(|defect| defect.problem == SourceProblem::AnchorReworded)
        .collect();
    assert!(
        reworded.is_empty(),
        "a crate has reworded a sentence this register quotes: {reworded:#?}"
    );
}

#[test]
fn every_judgement_is_recorded_inside_the_crate_it_is_attributed_to() {
    let report = reconcile(&register()).expect("the workspace is readable");
    let misplaced: Vec<_> = report
        .source_defects
        .iter()
        .filter(|defect| defect.problem == SourceProblem::LocusOutsideItsCrate)
        .collect();
    assert!(misplaced.is_empty(), "{misplaced:#?}");
}

#[test]
fn the_whole_reconciliation_is_clean() {
    let report = reconcile(&register()).expect("the workspace is readable");
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn the_backlog_parser_reads_ids_and_titles_and_ignores_prose() {
    // The two ids are assembled rather than written, for the reason the whole crate exists: a test
    // fixture under `crates/` is a citation as far as `tools/coverage.sh` is concerned, and
    // `crates/sweep`'s first attempt at this file cited the very ids it was asserting were uncited.
    let sample = format!(
        "# Remaining backlog\n\nSome prose that mentions a bullet.\n\n## Section heading\n\n\
         - `{}` Python Sdk\n- not a module line\n- `{}` Out Of Range Is Still Parsed As A Line\n",
        format_args!("{:02}.{:02}", 11, 4),
        format_args!("{:02}.{:02}", 99, 99),
    );
    let lines = parse_backlog(&sample);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].title, "Python Sdk");
    assert_eq!(lines[1].title, "Out Of Range Is Still Parsed As A Line");
}

#[test]
fn the_backlog_parser_does_not_mistake_a_version_string_for_a_module_line() {
    let lines = parse_backlog("- `1.0.4` a version\n- `abcde` five characters\n");
    assert!(lines.is_empty());
}

#[test]
fn a_backlog_that_parses_to_nothing_is_an_error_rather_than_an_empty_backlog() {
    // The failure `docs/BACKLOG.md` already suffered: a run that counted the file it had just
    // written produced an empty list, and an empty list read as "the work is done".
    let workspace = Workspace::here().expect("the workspace opens");
    let text = workspace
        .read(BACKLOG_PATH)
        .expect("the backlog is readable");
    assert!(
        !parse_backlog(&text).is_empty(),
        "the live backlog parsed to nothing, which means its format moved"
    );
}

#[test]
fn reconciliation_reads_the_workspace_as_text_and_takes_no_dependency_on_the_crates_it_checks() {
    // If this crate linked the fourteen crates it transcribes, it could not be built while any of
    // them was mid-edit — which is exactly when somebody is changing a classification.
    let manifest = include_str!("../Cargo.toml");
    for classified in [
        "bioprism-stewardship",
        "bioprism-atlashub",
        "bioprism-atlasx",
        "bioprism-devplat",
        "bioprism-ops",
        "bioprism-sweep",
        "bioprism-bioevalx",
        "bioprism-bioethics",
        "bioprism-metrics",
        "bioprism-trace",
        // The four crates that landed while this register was being written. Depending on any of
        // them would have made the register unbuildable during exactly the commit that emptied
        // three of its sections.
        "bioprism-dataops",
        "bioprism-interweave",
        "bioprism-megafactory",
    ] {
        assert!(
            !manifest.contains(classified),
            "`{classified}` is a dependency; the register must read it as text"
        );
    }
}

#[test]
fn the_reconciliation_survives_a_workspace_opened_by_the_caller() {
    let workspace = Workspace::here().expect("the workspace opens");
    let report = reconcile_in(&register(), &workspace).expect("readable");
    assert!(report.is_clean(), "{}", report.render());
}
