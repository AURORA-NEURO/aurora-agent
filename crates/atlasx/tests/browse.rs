//! Browsing is not evidence — blueprint 34.09.

mod common;

use bioprism_atlas::{FailureMechanism, Inducement, Severity, UnmeasuredReason};
use bioprism_atlasx::{
    audit, browse, browse_with_visibility, Answer, AtlasxError, BucketKey, Facet, Question,
    Surface, SurfaceCell, Unanswerable, Visibility,
};
use bioprism_hub::PublicationState;
use bioprism_metrics::{CapabilityGrid, GridCell, NoIntervalReason};
use common::{axes, capability, conditions, contested_record, grid, record};

fn three_records() -> Vec<bioprism_atlas::FailureRecord> {
    vec![
        record(
            "f1",
            "identity.lineage",
            FailureMechanism::StaleEvidenceTrusted,
            axes(Severity::Degraded, Inducement::ModelInduced),
        ),
        record(
            "f2",
            "identity.lineage",
            FailureMechanism::StaleEvidenceTrusted,
            axes(Severity::WrongConclusion, Inducement::ModelInduced),
        ),
        record(
            "f3",
            "cohort.statistics",
            FailureMechanism::PlanIgnoredKnownConstraint,
            axes(Severity::Degraded, Inducement::EvaluatorInduced),
        ),
    ]
}

#[test]
fn a_browse_refuses_a_rate_because_failures_carry_no_attempt_denominator() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert_eq!(
        browsed.answer(&Question::RatePerAttempt {
            capability: "identity.lineage".to_string()
        }),
        Answer::refused(Unanswerable::NoAttemptDenominator),
        "a set of failures says how many were recorded, not how many times it was tried"
    );
}

#[test]
fn the_rate_is_declared_and_refused_rather_than_omitted_from_the_declaration() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert!(browsed.declared().contains(&Question::RatePerAttempt {
        capability: "identity.lineage".to_string()
    }));
    let report = audit(&browsed);
    assert!(report
        .declared_but_refused
        .iter()
        .any(|(question, reason)| {
            matches!(question, Question::RatePerAttempt { .. })
                && *reason == Unanswerable::NoAttemptDenominator
        }));
}

#[test]
fn a_rate_becomes_answerable_only_when_a_measurement_supplies_the_denominator() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    let answer = browsed.rate_against(&grid("system-a"), &capability("identity.lineage"));
    let Some(SurfaceCell::Score { value }) = answer.cell() else {
        panic!("expected a score, got {answer:?}");
    };
    assert!((*value - 0.5).abs() < 1e-9, "two failures over four units");
}

#[test]
fn a_rate_refuses_a_grid_that_is_a_reading_of_a_different_subject() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert_eq!(
        browsed.rate_against(&grid("system-b"), &capability("identity.lineage")),
        Answer::refused(Unanswerable::DifferentSubject),
        "borrowing another system's attempt count is a coincidence of units"
    );
}

#[test]
fn a_rate_refuses_when_the_capabilitys_cell_is_a_hole() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert_eq!(
        browsed.rate_against(&grid("system-a"), &capability("causal.interpretation")),
        Answer::refused(Unanswerable::EvidenceAbsent)
    );
}

#[test]
fn a_rate_refuses_a_capability_the_grid_does_not_hold_at_all() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert_eq!(
        browsed.rate_against(&grid("system-a"), &capability("never.measured")),
        Answer::refused(Unanswerable::EvidenceAbsent)
    );
}

#[test]
fn an_evaluator_induced_failure_is_not_charged_to_the_system_in_a_rate() {
    let records = three_records();
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.evaluator_induced(), 1);
    let answer = browsed.rate_against(&grid("system-a"), &capability("cohort.statistics"));
    let Some(SurfaceCell::Score { value }) = answer.cell() else {
        panic!("expected a score");
    };
    assert_eq!(*value, 0.0, "the only failure there is a benchmark defect");
}

#[test]
fn a_contested_diagnosis_does_not_land_in_a_mechanism_bucket() {
    let records = vec![contested_record("f9", "identity.lineage")];
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.contested(), 1);
    assert!(browsed.bucket("contested").is_some());
    assert!(
        browsed
            .buckets()
            .iter()
            .all(|b| !matches!(b.key, BucketKey::Mechanism { .. })),
        "taking the mode at render time undoes the disagreement the record preserved"
    );
}

#[test]
fn the_modal_bucket_refuses_while_any_diagnosis_is_contested() {
    let mut records = three_records();
    records.push(contested_record("f9", "identity.lineage"));
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.modal_bucket(), Err(Unanswerable::LabelContested));
}

#[test]
fn the_modal_bucket_refuses_a_tie_rather_than_letting_the_sort_order_decide() {
    let records = vec![
        record(
            "f1",
            "a",
            FailureMechanism::StaleEvidenceTrusted,
            axes(Severity::Degraded, Inducement::ModelInduced),
        ),
        record(
            "f2",
            "a",
            FailureMechanism::PlanIgnoredKnownConstraint,
            axes(Severity::Degraded, Inducement::ModelInduced),
        ),
    ];
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.modal_bucket(), Err(Unanswerable::NotUnique));
}

#[test]
fn the_modal_bucket_refuses_an_empty_browse() {
    let browsed = browse("system-a", &[], Facet::Mechanism).expect("browses");
    assert_eq!(browsed.modal_bucket(), Err(Unanswerable::EvidenceAbsent));
}

#[test]
fn the_modal_bucket_answers_when_one_bucket_is_largest_and_nothing_is_contested() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    assert_eq!(
        browsed.modal_bucket(),
        Ok(&BucketKey::Mechanism {
            mechanism: FailureMechanism::StaleEvidenceTrusted
        })
    );
}

#[test]
fn withholding_a_record_shrinks_every_visible_share_and_none_of_the_denominator() {
    let records = three_records();
    let open = browse("system-a", &records, Facet::Mechanism).expect("browses");
    let closed = browse_with_visibility(
        "system-a",
        &records,
        Facet::Mechanism,
        &[Visibility::new("f1", PublicationState::UnderReview)],
    )
    .expect("browses");

    assert_eq!(open.records_browsed(), closed.records_browsed());
    assert_eq!(closed.withheld(), 1);
    assert_eq!(closed.visible(), 2);
    assert!(open.shares_sum_to_one());
    assert!(
        !closed.shares_sum_to_one(),
        "a page whose bars sum to less than the total is disclosing the withholding"
    );
}

#[test]
fn a_withheld_record_leaves_its_diagnosis_bucket_entirely() {
    let records = three_records();
    let closed = browse_with_visibility(
        "system-a",
        &records,
        Facet::Mechanism,
        &[Visibility::new("f1", PublicationState::Disputed)],
    )
    .expect("browses");
    let stale = closed
        .bucket("mechanism:stale_evidence_trusted")
        .expect("bucket survives");
    assert_eq!(
        stale.members,
        vec!["f2".to_string()],
        "a bucket label would disclose the diagnosis the state is withholding"
    );
    assert!(closed.bucket("withheld:disputed").is_some());
}

#[test]
fn a_withheld_bucket_renders_as_its_state_and_never_as_a_count() {
    let records = three_records();
    let closed = browse_with_visibility(
        "system-a",
        &records,
        Facet::Mechanism,
        &[Visibility::new("f1", PublicationState::Withdrawn)],
    )
    .expect("browses");
    let bucket = closed.bucket("withheld:withdrawn").expect("bucket exists");
    assert_eq!(
        bucket.cell(),
        SurfaceCell::withheld(PublicationState::Withdrawn)
    );
    assert!(bucket.cell().as_number().is_none());
}

#[test]
fn every_one_of_the_eight_non_available_states_survives_a_browse() {
    let records = three_records();
    for state in [
        PublicationState::Unavailable,
        PublicationState::Controlled,
        PublicationState::Stale,
        PublicationState::UnderReview,
        PublicationState::Disputed,
        PublicationState::Withdrawn,
        PublicationState::NonReproducible,
        PublicationState::NotComparable,
    ] {
        let closed = browse_with_visibility(
            "system-a",
            &records,
            Facet::Mechanism,
            &[Visibility::new("f1", state)],
        )
        .expect("browses");
        let bucket = closed
            .buckets()
            .iter()
            .find(|b| b.key.is_withheld())
            .unwrap_or_else(|| panic!("{state:?} produced no withheld bucket"));
        assert_eq!(bucket.cell(), SurfaceCell::withheld(state));
    }
}

#[test]
fn a_share_of_a_withheld_bucket_is_the_state_not_a_fraction() {
    let records = three_records();
    let closed = browse_with_visibility(
        "system-a",
        &records,
        Facet::Mechanism,
        &[Visibility::new("f1", PublicationState::Stale)],
    )
    .expect("browses");
    let answer = closed.answer(&Question::ShareOfAggregated {
        bucket: "withheld:stale".to_string(),
    });
    assert_eq!(
        answer.cell(),
        Some(&SurfaceCell::withheld(PublicationState::Stale))
    );
}

#[test]
fn a_browse_across_two_taxonomy_versions_is_refused() {
    let mut records = three_records();
    records[1].ontology_version = "bioprism-failure-taxonomy/0.2".to_string();
    assert!(matches!(
        browse("system-a", &records, Facet::Mechanism),
        Err(AtlasxError::MixedTaxonomyVersions { .. })
    ));
}

#[test]
fn a_repeated_failure_identifier_is_refused_rather_than_double_counted() {
    let mut records = three_records();
    records[1].failure_id = "f1".to_string();
    assert!(matches!(
        browse("system-a", &records, Facet::Mechanism),
        Err(AtlasxError::DuplicateRecord { .. })
    ));
}

#[test]
fn a_visibility_declaration_for_a_record_not_being_browsed_is_refused() {
    let records = three_records();
    assert!(matches!(
        browse_with_visibility(
            "system-a",
            &records,
            Facet::Mechanism,
            &[Visibility::new("absent", PublicationState::Stale)],
        ),
        Err(AtlasxError::VisibilityForAbsentRecord { .. })
    ));
}

#[test]
fn two_visibility_declarations_for_one_record_are_refused() {
    let records = three_records();
    assert!(matches!(
        browse_with_visibility(
            "system-a",
            &records,
            Facet::Mechanism,
            &[
                Visibility::new("f1", PublicationState::Stale),
                Visibility::new("f1", PublicationState::Withdrawn),
            ],
        ),
        Err(AtlasxError::DuplicateVisibility { .. })
    ));
}

#[test]
fn a_failure_with_no_architecture_component_goes_to_unattributed_not_to_a_plausible_one() {
    let mut records = three_records();
    records[0].axes = records[0].axes.clone().with_component("planner");
    let browsed = browse("system-a", &records, Facet::ArchitectureComponent).expect("browses");
    assert_eq!(
        browsed
            .bucket("component:unattributed")
            .expect("unattributed bucket")
            .len(),
        2
    );
    assert_eq!(
        browsed
            .bucket("component:planner")
            .expect("named bucket")
            .len(),
        1
    );
}

#[test]
fn a_bucket_names_its_members_so_a_bar_can_be_reproduced() {
    let browsed = browse("system-a", &three_records(), Facet::Severity).expect("browses");
    let degraded = browsed.bucket("severity:degraded").expect("bucket exists");
    assert_eq!(degraded.members, vec!["f1".to_string(), "f3".to_string()]);
}

#[test]
fn an_absent_bucket_is_refused_rather_than_answered_as_zero() {
    let browsed = browse("system-a", &three_records(), Facet::Severity).expect("browses");
    assert_eq!(
        browsed.answer(&Question::BucketCount {
            bucket: "severity:unsafe_action".to_string()
        }),
        Answer::refused(Unanswerable::NoSuchBucket),
        "a bucket that does not exist and one holding nothing are different facts"
    );
}

#[test]
fn a_browse_refuses_coverage_standing_and_comparison() {
    let browsed = browse("system-a", &three_records(), Facet::Mechanism).expect("browses");
    for question in [
        Question::ProfileCoverage,
        Question::CapabilityStanding {
            capability: "identity.lineage".to_string(),
        },
        Question::ComparisonWith {
            subject: "system-b".to_string(),
        },
    ] {
        assert_eq!(
            browsed.answer(&question),
            Answer::refused(Unanswerable::NotDeclared)
        );
    }
}

#[test]
fn a_browse_answers_only_what_it_declares() {
    let records = three_records();
    let browsed = browse_with_visibility(
        "system-a",
        &records,
        Facet::Mechanism,
        &[Visibility::new("f3", PublicationState::UnderReview)],
    )
    .expect("browses");
    let report = audit(&browsed);
    assert!(report.sound(), "{:?}", report.undeclared_but_answered);
}

#[test]
fn a_browse_is_deterministic_in_bucket_order() {
    let records = three_records();
    let first = browse("system-a", &records, Facet::Mechanism).expect("browses");
    let second = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(first, second);
    let labels: Vec<String> = first.buckets().iter().map(|b| b.key.label()).collect();
    let mut sorted = labels.clone();
    sorted.sort();
    assert_eq!(labels, sorted);
}

#[test]
fn a_browse_round_trips_through_json() {
    let browsed = browse("system-a", &three_records(), Facet::Inducement).expect("browses");
    let encoded = serde_json::to_string(&browsed).expect("serializes");
    let decoded: bioprism_atlasx::FailureBrowse =
        serde_json::from_str(&encoded).expect("deserializes");
    assert_eq!(browsed, decoded);
}

#[test]
fn distinct_families_counts_chains_without_dropping_a_record() {
    let records = three_records();
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.records_browsed(), 3);
    assert_eq!(
        browsed.distinct_families(),
        2,
        "two records share a chain; neither is discarded"
    );
    let counted: usize = browsed.buckets().iter().map(|b| b.len()).sum();
    assert_eq!(counted, 3, "no record leaves the browse");
}

#[test]
fn an_undiagnosed_failure_is_counted_but_localization_precision_is_not() {
    let mut records = three_records();
    records[0].axes.evidence_status = bioprism_atlas::EvidenceStatus::Lost;
    let browsed = browse("system-a", &records, Facet::Mechanism).expect("browses");
    assert_eq!(browsed.undiagnosed(), 1);
    assert!(
        !browsed
            .declared()
            .iter()
            .any(|q| matches!(q, Question::CapabilityStanding { .. })),
        "the section's localization-precision metric needs a ground truth no record carries"
    );
}

#[test]
fn a_rate_over_a_grid_cell_with_no_effective_size_refuses() {
    let records = three_records();
    let browsed = browse("system-thin", &records, Facet::Mechanism).expect("browses");
    let thin = CapabilityGrid::new("system-thin", conditions("system-thin")).with_cell(
        capability("identity.lineage"),
        GridCell::unmeasured(UnmeasuredReason::AllTrialsNonEvaluable),
    );
    assert_eq!(
        browsed.rate_against(&thin, &capability("identity.lineage")),
        Answer::refused(Unanswerable::EvidenceAbsent)
    );
}

#[test]
fn a_zero_effective_size_refuses_instead_of_dividing() {
    let records = three_records();
    let browsed = browse("system-zero", &records, Facet::Mechanism).expect("browses");
    let zero = CapabilityGrid::new("system-zero", conditions("system-zero")).with_cell(
        capability("identity.lineage"),
        GridCell::point(1.0, NoIntervalReason::EstimatorNotAvailable, 0).expect("cell"),
    );
    assert_eq!(
        browsed.rate_against(&zero, &capability("identity.lineage")),
        Answer::refused(Unanswerable::EvidenceAbsent)
    );
}
