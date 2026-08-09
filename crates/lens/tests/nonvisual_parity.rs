//! Cross-lens invariants — blueprint 42.27 and 42.31.
//!
//! The unit tests inside each lens check that lens. These check the properties that must hold
//! across *every* lens in the catalogue, because that is what a release gate actually rests on:
//! not that the leakage lens happens to be answerable without vision, but that no lens in this
//! crate can fail to be.
//!
//! Each test drives all six lenses through [`bioprism_lens::run`] with evidence chosen to produce
//! findings, then asserts a property of the sealed reports.

use bioprism_lens::{
    anytime::{AnytimeCurveLens, AnytimeEvaluation, CurvePoint, Stratum},
    catalogue, catalogue_ids,
    claim::{ClaimDossier, ClaimEvidenceLens, ClaimRecord, EvidenceItem},
    gate::ReleaseGate,
    leakage::{CohortLeakageLens, CohortSplit, SubjectRecord},
    missingness::{Missingness, Observation, Recorded, UnattemptedReason},
    profile::{ContextProfile, ContextTokenProfileLens, FactEntry},
    qc::{AssayPanel, AssayQcMissingnessLens, PanelCell},
    run,
    transport::{
        AssumptionStatus, CausalTransportLens, IdentificationStrategy, StratumSupport, StudyDesign,
        TransportQuestion,
    },
    Completeness, Layer, LensReport, RefusalReason, ReportOutcome,
};
use bioprism_scope::ScopeKey;

fn hole() -> Observation {
    Observation::Absent(Missingness::NeverMeasured {
        reason: UnattemptedReason::NotOrdered,
    })
}

fn subject(name: &str, split: &str, alias: &str) -> SubjectRecord {
    SubjectRecord {
        subject: name.into(),
        split: split.into(),
        aliases: vec![alias.to_string()],
        site: Recorded::known("MGH".to_string()),
        label_source_time: Recorded::known("2025-01-01T00:00:00Z".into()),
    }
}

/// One report from each of the six lenses, every one carrying at least one finding.
fn all_six_with_findings() -> Vec<LensReport> {
    let leakage = run(
        &CohortLeakageLens,
        &ScopeKey::new().exact("cohort", "C-1"),
        &CohortSplit::new(vec![
            subject("S001", "train", "ALT-77"),
            subject("S003", "test", "ALT-77"),
        ])
        .with_decision_time("2026-01-01T00:00:00Z"),
    )
    .expect("leakage lens runs");

    let claim = run(
        &ClaimEvidenceLens,
        &ScopeKey::new(),
        &ClaimDossier::new(vec![ClaimRecord::new(
            "PDL1 predicts response",
            ScopeKey::new().exact("population", "nsclc"),
        )
        .supported_by(EvidenceItem::new(
            "E1",
            "trial-A",
            ScopeKey::new().exact("population", "melanoma"),
        ))]),
    )
    .expect("claim lens runs");

    let mut question = TransportQuestion::new(
        "effect",
        ScopeKey::new().exact("population", "trial"),
        ScopeKey::new().exact("population", "community"),
        StudyDesign::Observational,
    )
    .with_identification(IdentificationStrategy::Backdoor)
    .with_strata(vec![StratumSupport {
        stratum: "age>=75".into(),
        target_units: 41,
        source_units: 0,
    }]);
    for assumption in CausalTransportLens::required_assumptions(StudyDesign::Observational) {
        question = question.assuming(assumption, AssumptionStatus::Unexamined);
    }
    let transport = run(
        &CausalTransportLens,
        &ScopeKey::new().exact("target_population", "community"),
        &question,
    )
    .expect("transport lens runs");

    let qc = run(
        &AssayQcMissingnessLens,
        &ScopeKey::new().exact("specimen", "SP-1"),
        &AssayPanel::new(vec![
            PanelCell::new("SP-1", "PDL1", hole()),
            PanelCell::new(
                "SP-1",
                "HER2",
                Observation::Absent(Missingness::BiologicalAbsence {
                    assay: "IHC".into(),
                }),
            ),
        ]),
    )
    .expect("qc lens runs");

    let profile = run(
        &ContextTokenProfileLens,
        &ScopeKey::new().exact("query", "Q-1"),
        &ContextProfile::new(
            vec![FactEntry::priced("germline_status", Layer::L2, 40, true)],
            Vec::new(),
        ),
    )
    .expect("profile lens runs");

    let anytime = run(
        &AnytimeCurveLens,
        &ScopeKey::new(),
        &AnytimeEvaluation::new(
            "eval-1",
            vec![Stratum::new("imaging", 100), Stratum::new("molecular", 80)],
        )
        .with_points(vec![CurvePoint::new("imaging", 40, 30)]),
    )
    .expect("anytime lens runs");

    vec![leakage, claim, transport, qc, profile, anytime]
}

#[test]
fn every_lens_in_the_catalogue_answers_without_a_rendering() {
    for report in all_six_with_findings() {
        let spoken = report.spoken();
        assert!(
            spoken.len() > 1,
            "{} produced no speakable answer",
            report.lens()
        );
        for line in &spoken {
            assert!(
                !line.trim().is_empty(),
                "{} spoke an empty line",
                report.lens()
            );
        }
    }
}

#[test]
fn every_witness_from_every_lens_is_a_row_of_named_columns() {
    for report in all_six_with_findings() {
        assert!(
            !report.witnesses().is_empty(),
            "{} was expected to find something",
            report.lens()
        );
        for row in report.witnesses() {
            assert_eq!(
                row.columns.len(),
                row.cells.len(),
                "{} produced a ragged row for {}",
                report.lens(),
                row.kind
            );
            assert!(!row.kind.is_empty());
            assert!(!row.sentence.is_empty());
            for field in row.spoken() {
                assert!(
                    !field.ends_with(':') && !field.ends_with(": "),
                    "{} produced an empty field in {}: {field}",
                    report.lens(),
                    row.kind
                );
            }
        }
    }
}

#[test]
fn every_witness_is_a_concrete_object_rather_than_a_score() {
    for report in all_six_with_findings() {
        for row in report.witnesses() {
            let named_something = row.cells.iter().any(|cell| {
                matches!(
                    cell,
                    bioprism_lens::Cell::Identifier { .. }
                        | bioprism_lens::Cell::Text { .. }
                        | bioprism_lens::Cell::Absent { .. }
                )
            });
            assert!(
                named_something,
                "{} produced a {} made only of numbers",
                report.lens(),
                row.kind
            );
        }
    }
}

#[test]
fn every_lens_produces_a_deterministic_receipt() {
    let first = all_six_with_findings();
    let second = all_six_with_findings();
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(
            a.receipt(),
            b.receipt(),
            "{} is not deterministic",
            a.lens()
        );
    }
}

#[test]
fn no_two_lenses_produce_the_same_receipt() {
    let reports = all_six_with_findings();
    for (i, a) in reports.iter().enumerate() {
        for b in reports.iter().skip(i + 1) {
            assert_ne!(a.receipt(), b.receipt());
        }
    }
}

#[test]
fn a_report_serializes_with_its_receipt_and_every_witness_sentence() {
    for report in all_six_with_findings() {
        let json = serde_json::to_string(&report).expect("report serializes");
        assert!(json.contains(report.receipt().as_str()));
        assert!(json.contains(report.blueprint_module()));
        for row in report.witnesses() {
            assert!(json.contains(&row.kind));
        }
    }
}

#[test]
fn a_gate_over_the_whole_catalogue_blocks_on_findings_and_names_every_one() {
    let gate = ReleaseGate::requiring(catalogue_ids())
        .blocking_on(vec![
            "identity_leakage",
            "scope_overreach",
            "positivity_violation",
            "unmeasured_quantity",
            "protected_fact_omitted",
            "uncovered_stratum",
        ])
        .permitting_partial();
    let outcome = gate.evaluate(&all_six_with_findings());
    assert!(!outcome.passed());
    let lenses: std::collections::BTreeSet<&str> =
        outcome.blocks.iter().map(|b| b.lens()).collect();
    assert_eq!(
        lenses.len(),
        6,
        "every lens contributed a block: {lenses:?}"
    );
    assert!(outcome.established().len() >= 6);
}

#[test]
fn a_gate_that_was_never_run_never_reads_as_a_gate_that_passed() {
    let gate = ReleaseGate::requiring(catalogue_ids());
    let nothing_ran = gate.evaluate(&[]);
    assert!(!nothing_ran.passed());
    assert_eq!(nothing_ran.unchecked().len(), 6);
    assert!(nothing_ran.established().is_empty());
    for block in &nothing_ran.blocks {
        assert_eq!(block.kind(), "lens_not_run");
        assert!(block.sentence().contains("never run"));
    }
}

#[test]
fn a_truncated_evaluation_blocks_a_gate_that_does_not_permit_partial_answers() {
    let anytime = run(
        &AnytimeCurveLens,
        &ScopeKey::new(),
        &AnytimeEvaluation::new(
            "eval-1",
            vec![Stratum::new("imaging", 100), Stratum::new("molecular", 80)],
        )
        .with_points(vec![CurvePoint::new("imaging", 40, 30)]),
    )
    .unwrap();
    assert_eq!(
        anytime.completeness(),
        Completeness::Partial {
            examined: 1,
            eligible: 2
        }
    );
    let strict = ReleaseGate::requiring(vec![bioprism_lens::LensId::new(AnytimeCurveLens::ID)]);
    let outcome = strict.evaluate(std::slice::from_ref(&anytime));
    assert_eq!(outcome.blocks[0].kind(), "answer_incomplete");
    assert!(outcome.blocks[0].sentence().contains("molecular"));
    assert!(strict.permitting_partial().evaluate(&[anytime]).passed());
}

#[test]
fn a_lens_that_cannot_see_its_evidence_is_not_reported_as_a_lens_that_found_nothing() {
    let empty_cohort = run(
        &CohortLeakageLens,
        &ScopeKey::new().exact("cohort", "C-1"),
        &CohortSplit::new(Vec::new()),
    )
    .unwrap();
    let clean_cohort = run(
        &CohortLeakageLens,
        &ScopeKey::new().exact("cohort", "C-1"),
        &CohortSplit::new(vec![
            subject("S001", "train", "A1"),
            subject("S002", "test", "B1"),
        ])
        .with_decision_time("2026-01-01T00:00:00Z"),
    )
    .unwrap();

    assert!(matches!(
        empty_cohort.outcome(),
        ReportOutcome::EvidenceAbsent(_)
    ));
    assert!(clean_cohort.is_answered());
    assert!(clean_cohort.witnesses().is_empty());
    assert_ne!(empty_cohort.receipt(), clean_cohort.receipt());

    let gate = ReleaseGate::requiring(vec![bioprism_lens::LensId::new(CohortLeakageLens::ID)]);
    assert!(!gate.evaluate(&[empty_cohort]).passed());
    assert!(gate.evaluate(&[clean_cohort]).passed());
}

#[test]
fn every_declared_refusal_reason_in_the_catalogue_is_reachable_in_principle() {
    let declared: std::collections::BTreeSet<RefusalReason> = catalogue()
        .iter()
        .flat_map(|d| d.refuses().to_vec())
        .collect();
    assert!(declared.contains(&RefusalReason::ScopePreconditionUnmet));
    assert!(declared.contains(&RefusalReason::WouldAggregateIncomparableScopes));
    assert!(declared.contains(&RefusalReason::WouldRequireInterventionalClaim));
    assert!(declared.contains(&RefusalReason::NoAnswerableFormulation));
}

#[test]
fn a_lens_with_an_unbound_precondition_refuses_and_the_refusal_names_the_dimension() {
    for declaration in catalogue() {
        if declaration.preconditions().is_empty() {
            continue;
        }
        let unmet = declaration.unmet_preconditions(&ScopeKey::new());
        assert_eq!(
            unmet.len(),
            declaration.preconditions().len(),
            "{} did not report its own unbound dimensions",
            declaration.id()
        );
        for precondition in unmet {
            assert!(!precondition.why.is_empty());
        }
    }
}
