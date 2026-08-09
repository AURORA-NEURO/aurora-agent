//! Answering "why was this fact omitted" without re-running the compiler.
//!
//! The brief's test for this module is whether an agent holding only the record can answer the
//! question, and whether the answers preserve every distinction `bioprism-section` makes. These
//! tests assemble a record resembling a real compile and interrogate it.

use bioprism_devx::catalogue::diagnose_record;
use bioprism_devx::diagnostic::Certainty;
use bioprism_devx::introspect::{
    CompileRecord, DeveloperQuestion, OmissionAnswer, OmissionEntry, PassOutcome, PassRecord,
};
use bioprism_devx::lint::lint;
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};

fn compiled() -> CompileRecord {
    let mut manifest = OmissionManifest::default();
    manifest.push(OmissionGroup {
        reason: "no dependency path reaches the target".into(),
        influence: InfluenceClass::Zero,
        count: 118,
        bound: None,
        examples: vec!["fact:unreachable-7".into()],
    });
    manifest.push(OmissionGroup {
        reason: "governed by an event not released at the decision cut".into(),
        influence: InfluenceClass::DeferredAcquisition,
        count: 6,
        bound: None,
        examples: vec!["fact:post-cut-3".into()],
    });
    manifest.push(OmissionGroup {
        reason: "not analysed".into(),
        influence: InfluenceClass::Unknown,
        count: 2,
        bound: None,
        examples: vec!["fact:unchecked-1".into()],
    });

    CompileRecord::new("world:reference-cohort", "query:split-integrity")
        .selecting("fact:selected-1")
        .selecting("fact:selected-2")
        .with_manifest(manifest)
        .with_pass(
            PassRecord::new(
                "protected_closure",
                PassOutcome::Ran {
                    considered: 140,
                    retained: 22,
                },
                "unioned every fact carrying a protected tag into the selection",
            )
            .removing("fact:pruned-by-closure"),
        )
        .with_pass(PassRecord::new(
            "temporal_accessibility",
            PassOutcome::Ran {
                considered: 22,
                retained: 16,
            },
            "withheld every fact governed by an event later than the cut",
        ))
        .with_pass(PassRecord::new(
            "obstruction_tests",
            PassOutcome::Deferred {
                reason: "fiber-world/0.1 carries no cover, so no two local sections overlap".into(),
            },
            "no cover was declared, so no obstruction could be computed",
        ))
        .with_pass(PassRecord::new(
            "rate_distortion",
            PassOutcome::Skipped {
                unmet_precondition: "the query declares no decision_loss to trade distortion \
                                     against"
                    .into(),
            },
            "no loss function was available",
        ))
        .bound_by("b".repeat(64))
        .limited_by("the oracle derives status solely from whether the witness list is empty")
}

#[test]
fn the_record_answers_every_developer_question_it_carries_data_for() {
    let coverage = compiled().coverage();
    for question in DeveloperQuestion::ALL {
        assert!(
            coverage.answers(question),
            "{} is unanswerable from a full record",
            question.as_str()
        );
    }
    assert!(coverage.unanswerable.is_empty());
}

#[test]
fn a_zero_influence_omission_and_an_unknown_influence_omission_never_read_alike() {
    let record = compiled();
    let zero = record.why_omitted("fact:unreachable-7");
    let unknown = record.why_omitted("fact:unchecked-1");
    match (zero, unknown) {
        (
            OmissionAnswer::Omitted {
                influence: a,
                reason: reason_a,
                ..
            },
            OmissionAnswer::Omitted {
                influence: b,
                reason: reason_b,
                ..
            },
        ) => {
            assert_eq!(a, InfluenceClass::Zero);
            assert_eq!(b, InfluenceClass::Unknown);
            assert_ne!(reason_a, reason_b);
            assert_ne!(
                OmissionEntry::developer_label(a),
                OmissionEntry::developer_label(b)
            );
            assert!(a.supports_sufficiency());
            assert!(!b.supports_sufficiency());
        }
        other => panic!("expected two omissions, got {other:?}"),
    }
}

#[test]
fn the_deferred_acquisition_class_is_not_folded_into_unknown() {
    match compiled().why_omitted("fact:post-cut-3") {
        OmissionAnswer::Omitted { influence, .. } => {
            assert_eq!(influence, InfluenceClass::DeferredAcquisition);
            assert!(!influence.supports_sufficiency());
        }
        other => panic!("expected an omission, got {other:?}"),
    }
}

#[test]
fn a_subject_nobody_recorded_is_answered_as_unrecorded_and_not_as_provably_irrelevant() {
    let answer = compiled().why_omitted("fact:who-knows");
    assert!(!answer.is_answer_about_the_world());
    match answer {
        OmissionAnswer::NotRecorded { because } => {
            assert!(because.contains("counts and representative members"));
        }
        other => panic!("expected not_recorded, got {other:?}"),
    }
}

#[test]
fn an_omission_attributed_only_to_a_pass_is_marked_inferred_rather_than_observed() {
    match compiled().why_omitted("fact:pruned-by-closure") {
        OmissionAnswer::Omitted {
            certainty,
            attributed_to,
            influence,
            ..
        } => {
            assert_eq!(certainty, Certainty::Inferred);
            assert_eq!(attributed_to.as_deref(), Some("protected_closure"));
            assert_eq!(influence, InfluenceClass::Unknown);
        }
        other => panic!("expected an omission, got {other:?}"),
    }
}

#[test]
fn a_selected_subject_is_answered_from_the_authoritative_set() {
    let record = compiled();
    assert_eq!(
        record.why_omitted("fact:selected-1"),
        OmissionAnswer::Selected
    );
    assert_eq!(
        record.why_omitted("fact:selected-2"),
        OmissionAnswer::Selected
    );
}

#[test]
fn a_deferred_pass_and_a_skipped_pass_are_distinguishable_and_both_explain_themselves() {
    let record = compiled();
    let deferred = record.pass("obstruction_tests").expect("present");
    let skipped = record.pass("rate_distortion").expect("present");
    assert_eq!(deferred.outcome.as_str(), "deferred");
    assert_eq!(skipped.outcome.as_str(), "skipped");
    assert!(deferred
        .outcome
        .absence_reason()
        .expect("reason")
        .contains("no cover"));
    assert!(skipped
        .outcome
        .absence_reason()
        .expect("reason")
        .contains("decision_loss"));
}

#[test]
fn the_record_reports_two_passes_that_ran_and_two_that_did_not() {
    let record = compiled();
    assert_eq!(record.ran().len(), 2);
    assert_eq!(record.did_not_run().len(), 2);
}

#[test]
fn the_sufficiency_verdict_is_the_manifests_and_the_blocking_groups_carry_remedies() {
    let record = compiled();
    assert!(!record.supports_sufficiency_claim());
    let blocking = record.blocking_omissions();
    assert_eq!(blocking.len(), 2);
    for entry in blocking {
        assert!(
            entry.remedy.is_some(),
            "{:?} blocks sufficiency and offers no way out",
            entry.influence
        );
    }
}

#[test]
fn the_only_group_supporting_sufficiency_offers_no_remedy_because_none_is_needed() {
    let supporting: Vec<OmissionEntry> = compiled()
        .omissions()
        .into_iter()
        .filter(|entry| entry.supports_sufficiency)
        .collect();
    assert_eq!(supporting.len(), 1);
    assert_eq!(supporting[0].influence, InfluenceClass::Zero);
    assert!(supporting[0].remedy.is_none());
}

#[test]
fn a_record_with_a_certificate_cites_the_digest_as_the_site_of_anything_it_reports() {
    let record = compiled();
    let site = record.site();
    assert!(site.is_addressable());
    assert!(site.describe().starts_with("sha256:"));
    assert!(record.verified_digest("what bound this").is_ok());
}

#[test]
fn the_diagnostics_derived_from_this_record_name_the_unknown_group_and_nothing_else() {
    let record = compiled();
    let produced = diagnose_record(&record);
    assert_eq!(produced.len(), 1);
    assert_eq!(produced[0].code.as_str(), "DEVX-0002");
    assert!(produced[0].observed.contains("2 subjects"));
    assert!(lint(&produced).is_clean());
}

#[test]
fn a_record_that_answers_nothing_says_which_questions_it_cannot_answer() {
    let coverage = CompileRecord::new("w", "q").coverage();
    assert_eq!(coverage.answerable.len(), 1);
    assert!(coverage.answers(DeveloperQuestion::WhetherTheContextClaimsSufficiency));
    assert_eq!(coverage.unanswerable.len(), 5);
    for (_, why) in &coverage.unanswerable {
        assert!(!why.trim().is_empty());
    }
}
