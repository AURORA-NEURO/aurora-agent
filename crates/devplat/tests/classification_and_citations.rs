//! The classification is the crate's headline claim, and the citation rule is the crate's promise
//! not to inflate coverage. Both are checked here against the crate's own source tree.

use bioprism_devplat::citations::{audit, scan};
use bioprism_devplat::classify::{
    classification, implemented_module_ids, not_implemented, verdict_counts, Verdict,
};
use bioprism_devplat::surface::{foreign_subjects, Locale};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn devplat_dir() -> PathBuf {
    bioprism_cookbook::verify::workspace_root()
        .join("crates")
        .join("devplat")
}

#[test]
fn twenty_modules_are_classified_and_each_title_appears_once() {
    let rows = classification();
    assert_eq!(rows.len(), 20, "fourteen plus six were left uncited");
    let titles: BTreeSet<&str> = rows.iter().map(|row| row.title).collect();
    assert_eq!(titles.len(), 20, "a title was classified twice");
}

#[test]
fn the_four_buckets_are_three_seven_six_and_four() {
    assert_eq!(verdict_counts(), [3, 7, 6, 4]);
    assert_eq!(verdict_counts().iter().sum::<usize>(), classification().len());
}

#[test]
fn only_an_implemented_verdict_can_carry_a_module_id() {
    for row in classification() {
        let has_id = row.verdict.module_id().is_some();
        let is_implemented = matches!(row.verdict, Verdict::ImplementedHere { .. });
        assert_eq!(
            has_id, is_implemented,
            "`{}` carries an id without implementing anything",
            row.title
        );
    }
}

#[test]
fn this_crate_cites_exactly_the_four_modules_it_implemented() {
    let declared = implemented_module_ids();
    let report = audit(&devplat_dir(), declared.iter().copied()).expect("the crate directory reads");
    assert!(
        report.files_scanned > 8,
        "the audit read {} files, which is too few to have seen the crate",
        report.files_scanned
    );
    assert!(
        report.is_clean(),
        "citation defects: {:?}",
        report.defects()
    );
}

#[test]
fn every_declared_module_id_actually_appears_in_the_source() {
    let declared = implemented_module_ids();
    assert_eq!(declared.len(), 4);
    let report = audit(&devplat_dir(), declared.iter().copied()).expect("the crate directory reads");
    assert!(
        report.uncited.is_empty(),
        "declared but never written down, so coverage cannot see it: {:?}",
        report.uncited
    );
}

#[test]
fn the_scanner_accepts_a_module_id_and_rejects_its_near_misses() {
    assert_eq!(scan("implements 11.23 here"), BTreeSet::from(["11.23".into()]));
    assert!(scan("version 0.1.0").is_empty(), "a semver is not a citation");
    assert!(scan("111.23").is_empty(), "the section must not abut a digit");
    assert!(scan("11.234").is_empty(), "the module must not abut a digit");
    assert!(scan("x11.23").is_empty(), "the token must not abut a letter");
    assert!(scan("Python 3.12").is_empty(), "a one-digit section is not one");
    assert!(scan("00.12").is_empty(), "section zero is outside the range");
    assert!(scan("50.12").is_empty(), "section fifty is outside the range");
    let ends = format!("{}.{} and {}.{}", 49, 99, "01", "00");
    assert_eq!(scan(&ends).len(), 2, "sections one and forty-nine are inside");
}

#[test]
fn a_two_decimal_percentage_in_the_section_range_scans_as_a_citation() {
    let percentage = format!("{}.{}% furniture", 34, 61);
    assert_eq!(
        scan(&percentage).len(),
        1,
        "this is the trap the crate rounds its figures to avoid"
    );
    let rounded = format!("{}.{}% furniture", 34, 6);
    assert!(scan(&rounded).is_empty(), "one decimal place is safe");
}

#[test]
fn the_citation_audit_refuses_something_that_is_not_a_directory() {
    let file = devplat_dir().join("Cargo.toml");
    assert!(audit(&file, ["11.23"]).is_err());
}

#[test]
fn the_foreign_census_and_the_classification_name_the_same_seven_subjects() {
    let census: BTreeSet<&str> = foreign_subjects()
        .into_iter()
        .map(|subject| subject.title)
        .collect();
    let classified: BTreeSet<&str> = classification()
        .into_iter()
        .filter(|row| matches!(row.verdict, Verdict::ForeignArtifact { .. }))
        .map(|row| row.title)
        .collect();
    assert_eq!(census.len(), 7);
    assert_eq!(census, classified);
}

#[test]
fn every_foreign_subject_is_outside_this_repository() {
    for subject in foreign_subjects() {
        assert_eq!(
            subject.surface.locale(),
            Locale::OutsideRepository,
            "`{}` was classified foreign but its surface is in the tree",
            subject.title
        );
        assert!(
            !subject.why_not_here.trim().is_empty(),
            "`{}` gives no reason",
            subject.title
        );
    }
}

#[test]
fn sixteen_modules_are_named_by_title_with_a_reason_and_never_by_id() {
    let rows = not_implemented();
    assert_eq!(rows.len(), 16);
    for (title, bucket, because) in rows {
        assert!(scan(title).is_empty(), "`{title}` contains a citation token");
        assert!(scan(because).is_empty(), "the reason for `{title}` cites an id");
        assert!(
            because.len() > 80,
            "`{title}` is dismissed in {} characters, which is an assertion not a reason",
            because.len()
        );
        assert!(
            ["process", "foreign artifact", "covered elsewhere"].contains(&bucket),
            "unexpected bucket `{bucket}`"
        );
    }
}

#[test]
fn every_covered_elsewhere_verdict_names_at_least_one_crate() {
    let mut seen = 0;
    for row in classification() {
        if let Verdict::CoveredElsewhere { crates, .. } = &row.verdict {
            seen += 1;
            assert!(
                !crates.is_empty(),
                "`{}` claims coverage elsewhere without naming it",
                row.title
            );
            for name in crates {
                assert!(
                    name.starts_with("bioprism-"),
                    "`{name}` is not a workspace package name"
                );
            }
        }
    }
    assert_eq!(seen, 6);
}

#[test]
fn every_implemented_verdict_names_a_module_of_this_crate() {
    for row in classification() {
        if let Verdict::ImplementedHere { rust_module, .. } = row.verdict {
            assert!(
                rust_module.starts_with("bioprism_devplat::"),
                "`{rust_module}` is not a module of this crate"
            );
        }
    }
}

#[test]
fn the_module_ids_are_sorted_deduplicated_and_four() {
    let ids = implemented_module_ids();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(ids, sorted);
    assert_eq!(ids.len(), 4);
}
