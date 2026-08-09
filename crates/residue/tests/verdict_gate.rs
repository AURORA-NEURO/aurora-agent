//! The construction gate: what cannot be said, and why each refusal is the point.
//!
//! Every test here asserts that some *unsourced* or *self-contradictory* claim has no
//! representation. A test that a rule holds is good; a type that makes the rule unbreakable is
//! better, and these tests exist to fail loudly if a later refactor softens one of the gates into a
//! validation that a caller can skip.

use bioprism_residue::{
    Classification, Entry, ForeignSurface, ModuleKey, Register, RegisterError, Source, Standing,
    UncoveredStanding, Verdict, VerdictError,
};

fn good_source() -> Source {
    Source::transcribed(
        "bioprism-stewardship",
        "crates/stewardship/src/lib.rs",
        "Twelve do not. They describe councils",
        "Councils, votes and cadence describe what people do rather than what an artifact holds.",
    )
    .expect("a well-formed source")
}

#[test]
fn a_verdict_without_a_recorded_source_is_not_constructible() {
    let source = good_source();
    let verdict = Verdict::record(Classification::Process, source).expect("licensed");
    assert_eq!(verdict.recorded_by().as_str(), "bioprism-stewardship");
    assert!(!verdict.source().reasoning().is_empty());
    assert!(!verdict.source().anchor().needle.is_empty());
}

#[test]
fn a_source_with_a_one_word_argument_is_refused_because_a_label_is_not_a_reason() {
    let error = Source::transcribed(
        "bioprism-ops",
        "crates/ops/src/lib.rs",
        "Reference Technology Baseline | process",
        "process",
    )
    .expect_err("a label is not an argument");
    assert!(matches!(error, VerdictError::ReasoningTooThin { .. }));
}

#[test]
fn an_anchor_short_enough_to_occur_by_accident_is_refused() {
    let error = Source::transcribed(
        "bioprism-ops",
        "crates/ops/src/lib.rs",
        "process",
        "A fragment this short would match somewhere in almost any file in the workspace.",
    )
    .expect_err("a three-word needle witnesses nothing");
    assert!(matches!(error, VerdictError::AnchorTooThin { .. }));
}

#[test]
fn a_judgement_recorded_in_no_file_has_no_locus_and_is_refused() {
    let error = Source::transcribed(
        "bioprism-ops",
        "   ",
        "Reference Technology Baseline | process",
        "Without a file, nothing can check that the crate still says what is attributed to it.",
    )
    .expect_err("a judgement recorded nowhere cannot be resolved");
    assert!(matches!(error, VerdictError::LocusMissing));
}

#[test]
fn a_locus_written_with_backslashes_is_refused_so_paths_compare_against_the_manifest() {
    let error = Source::transcribed(
        "bioprism-ops",
        "crates\\ops\\src\\lib.rs",
        "Reference Technology Baseline | process",
        "The workspace manifest is forward-slashed and the comparison has to be exact.",
    )
    .expect_err("a native path would never match a manifest entry");
    assert!(matches!(
        error,
        VerdictError::LocusNotWorkspaceRelative { .. }
    ));
}

#[test]
fn discharged_elsewhere_naming_no_crate_is_not_constructible() {
    let empty: [&str; 0] = [];
    let error = Classification::discharged_by(empty).expect_err("a discharge needs a referent");
    assert!(matches!(error, VerdictError::NoDischarger));
}

#[test]
fn nobody_has_read_it_with_no_survey_is_not_constructible() {
    let empty: [&str; 0] = [];
    let error =
        UncoveredStanding::nobody_has_read(empty).expect_err("an empty survey is not a search");
    assert!(matches!(error, VerdictError::EmptySurvey));
}

#[test]
fn real_work_not_done_with_no_stated_blocker_is_not_constructible() {
    let error = UncoveredStanding::real_work_not_done("no time")
        .expect_err("a blocker nobody stated cannot be told from an untouched module");
    assert!(matches!(error, VerdictError::NoBlocker));
}

#[test]
fn a_block_level_split_that_names_neither_side_of_the_division_is_refused() {
    let error = Classification::block_level_split(["bioprism-metrics"], "prose")
        .expect_err("a split with no residue names nothing");
    assert!(matches!(error, VerdictError::NoBlocks));
}

#[test]
fn this_register_cannot_transcribe_a_judgement_from_itself() {
    let source = Source::transcribed(
        "bioprism-residue",
        "crates/residue/src/register.rs",
        "A survey that found nothing is still a finding",
        "A source pointing back at the file making the claim is the shape of every unsourced one.",
    )
    .expect("the source itself is well formed");
    let error = Verdict::record(Classification::Process, source)
        .expect_err("self-transcription is circular");
    assert!(matches!(error, VerdictError::TranscribedByItself { .. }));
}

#[test]
fn an_absence_of_judgement_cannot_be_transcribed_from_a_judgement() {
    let classification = Classification::GenuinelyUncovered {
        standing: UncoveredStanding::nobody_has_read(["bioprism-fabric"]).expect("one crate"),
    };
    let source = Source::transcribed(
        "bioprism-fabric",
        "crates/fabric/src/lib.rs",
        "Modules of §23 skipped as prose",
        "If no crate recorded a judgement, no crate can have stated the verdict saying so.",
    )
    .expect("the source itself is well formed");
    let error = Verdict::record(classification, source).expect_err("the one contradictory pairing");
    assert!(matches!(error, VerdictError::AbsenceCannotBeTranscribed));
}

#[test]
fn the_same_absence_is_constructible_when_it_is_labelled_as_this_registers_reading() {
    let classification = Classification::GenuinelyUncovered {
        standing: UncoveredStanding::nobody_has_read(["bioprism-fabric"]).expect("one crate"),
    };
    let source = Source::inferred(
        "bioprism-residue",
        "crates/residue/src/register.rs",
        "A survey that found nothing is still a finding",
        "The absence is this register's own reading of the workspace and its standing says so.",
    )
    .expect("well formed");
    let verdict = Verdict::record(classification, source).expect("licensed");
    assert_eq!(verdict.standing(), Standing::InferredHere);
}

#[test]
fn an_entry_with_no_verdict_is_the_backlog_line_it_was_supposed_to_replace_and_is_refused() {
    let key = ModuleKey::new(14, 1).expect("in range");
    let error = Entry::new(key, "Project Governance", Vec::new())
        .expect_err("an entry with no explanation explains nothing");
    assert!(matches!(error, RegisterError::NoVerdict { .. }));
}

#[test]
fn an_entry_with_no_title_has_no_key_because_the_register_is_keyed_by_title() {
    let key = ModuleKey::new(14, 1).expect("in range");
    let error = Entry::new(
        key,
        "  ",
        vec![Verdict::record(Classification::Process, good_source()).expect("licensed")],
    )
    .expect_err("an untitled module cannot be looked up");
    assert!(matches!(error, RegisterError::TitleMissing));
}

#[test]
fn a_module_key_outside_the_range_the_coverage_script_reads_is_refused() {
    assert!(matches!(
        ModuleKey::new(50, 1),
        Err(RegisterError::SectionOutOfRange { section: 50 })
    ));
    assert!(matches!(
        ModuleKey::new(0, 1),
        Err(RegisterError::SectionOutOfRange { section: 0 })
    ));
    assert!(matches!(
        ModuleKey::new(11, 0),
        Err(RegisterError::IndexOutOfRange { index: 0 })
    ));
}

#[test]
fn a_module_id_is_assembled_from_components_and_zero_padded_on_both_sides() {
    let key = ModuleKey::new(4, 2).expect("in range");
    assert_eq!(key.id().len(), 5);
    assert!(key.id().starts_with("04"));
    assert!(key.id().ends_with("02"));
    assert_eq!(key.section_label(), "§04");
}

#[test]
fn two_modules_cannot_share_one_blueprint_id() {
    let verdict = Verdict::record(Classification::Process, good_source()).expect("licensed");
    let key = ModuleKey::new(14, 1).expect("in range");
    let entries = vec![
        Entry::new(key, "Project Governance", vec![verdict.clone()]).expect("well formed"),
        Entry::new(key, "Something Else Entirely", vec![verdict]).expect("well formed"),
    ];
    assert!(matches!(
        Register::new(entries),
        Err(RegisterError::DuplicateKey)
    ));
}

#[test]
fn two_modules_cannot_share_one_title_because_the_register_is_looked_up_by_it() {
    let verdict = Verdict::record(Classification::Process, good_source()).expect("licensed");
    let entries = vec![
        Entry::new(
            ModuleKey::new(14, 1).expect("in range"),
            "Project Governance",
            vec![verdict.clone()],
        )
        .expect("well formed"),
        Entry::new(
            ModuleKey::new(34, 1).expect("in range"),
            "project governance",
            vec![verdict],
        )
        .expect("well formed"),
    ];
    assert!(matches!(
        Register::new(entries),
        Err(RegisterError::DuplicateTitle { .. })
    ));
}

#[test]
fn a_foreign_artifact_carries_the_surface_it_lives_on_rather_than_a_bare_flag() {
    let verdict = Verdict::record(
        Classification::ForeignArtifact {
            surface: ForeignSurface::GitHubAction,
        },
        good_source(),
    )
    .expect("licensed");
    match verdict.classification() {
        Classification::ForeignArtifact { surface } => {
            assert_eq!(surface.as_str(), "github action");
        }
        other => panic!("expected a foreign artifact, got {other:?}"),
    }
}

#[test]
fn only_genuinely_uncovered_reports_work_remaining() {
    let uncovered = Classification::GenuinelyUncovered {
        standing: UncoveredStanding::nobody_has_read(["bioprism-fabric"]).expect("one crate"),
    };
    assert!(uncovered.is_work_remaining());
    assert!(!Classification::Process.is_work_remaining());
    assert!(!Classification::discharged_by(["bioprism-mcp"])
        .expect("one crate")
        .is_work_remaining());
    assert!(!Classification::ForeignArtifact {
        surface: ForeignSurface::CiWorkflow
    }
    .is_work_remaining());
}
