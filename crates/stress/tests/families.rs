//! What each stress family promises, checked against what it does.
//!
//! Every test names the claim it defends. A test called `prevalence_shift_works` would pass for a
//! generator that quietly rewrote the measurements, which is the failure blueprint 32.07 spends
//! its failure-risk section on.

use bioprism_stress::{
    apply, declare, perturb, Cohort, ConclusionValue, Knob, Magnitude, Obligation, Procedure,
    Stress, StressError, StressRelation, Subject,
};

const SEED: u64 = 20_260_808;

fn balanced() -> Cohort {
    Cohort::synthetic("cohort-balanced", SEED, 40, 0.4, 1.5)
}

/// A cohort whose site-b holds three quarters of the positives and a quarter of the negatives.
///
/// Enriched but not pure, so a batch shift is still identifiable — which is the configuration in
/// which "the conclusion was reading the batch" is a statement about the conclusion rather than an
/// artefact of a cohort nobody could have analysed.
fn batch_enriched() -> Cohort {
    let mut cohort = Cohort::synthetic("cohort-enriched", SEED, 40, 0.4, 1.5);
    for (index, subject) in cohort.subjects.iter_mut().enumerate() {
        let minority = index % 4 == 0;
        subject.batch = match (subject.condition, minority) {
            (true, true) | (false, false) => "site-a".into(),
            _ => "site-b".into(),
        };
    }
    cohort
}

fn full(id: &str, knob: Knob) -> Stress {
    Stress::new(id, knob, Magnitude::FULL, SEED)
}

fn scalar(procedure: &Procedure, cohort: &Cohort) -> f64 {
    procedure
        .conclude(cohort)
        .expect("procedure is defined on this cohort")
        .value
        .as_scalar()
        .expect("procedure yields a scalar")
}

#[test]
fn reweighting_moves_the_base_rate_without_touching_a_single_measurement() {
    let parent = balanced();
    let stress = full(
        "to-deployment",
        Knob::PrevalenceShift {
            target_prevalence: 0.05,
        },
    );
    let stressed = apply(&parent, &stress).expect("prevalence shift applies");

    assert!((stressed.prevalence() - 0.05).abs() < 1e-12);
    for (before, after) in parent.subjects.iter().zip(stressed.subjects.iter()) {
        assert_eq!(before.marker, after.marker);
        assert_eq!(before.volume_mm3, after.volume_mm3);
        assert_eq!(before.condition, after.condition);
    }
}

#[test]
fn a_discriminative_ranking_does_not_move_under_prevalence_shift() {
    let parent = balanced();
    let stress = full(
        "to-deployment",
        Knob::PrevalenceShift {
            target_prevalence: 0.02,
        },
    );
    let stressed = apply(&parent, &stress).expect("prevalence shift applies");

    let declared = declare(&stress, &Procedure::MarkerRanking, &parent).expect("relation declared");
    assert_eq!(declared.obligation, Obligation::Required);
    let before = Procedure::MarkerRanking.conclude(&parent).unwrap();
    let after = Procedure::MarkerRanking.conclude(&stressed).unwrap();
    assert!(declared.relation.check(&before, &after).held());
}

#[test]
fn rank_concordance_is_algebraically_invariant_under_class_uniform_reweighting() {
    let parent = balanced();
    for target in [0.02, 0.1, 0.5, 0.9] {
        let stress = full(
            "shift",
            Knob::PrevalenceShift {
                target_prevalence: target,
            },
        );
        let stressed = apply(&parent, &stress).expect("prevalence shift applies");
        let before = scalar(&Procedure::MarkerSeparation, &parent);
        let after = scalar(&Procedure::MarkerSeparation, &stressed);
        assert!(
            (before - after).abs() < 1e-12,
            "separation moved from {before} to {after} at target {target}"
        );
    }
}

#[test]
fn a_calibrated_posterior_moves_by_exactly_the_change_in_prior_log_odds() {
    let parent = balanced();
    let procedure = Procedure::CalibratedLogOdds {
        slope: 1.0,
        reference: 0.0,
    };
    for target in [0.02, 0.2, 0.8] {
        let stress = full(
            "shift",
            Knob::PrevalenceShift {
                target_prevalence: target,
            },
        );
        let stressed = apply(&parent, &stress).expect("prevalence shift applies");
        let declared = declare(&stress, &procedure, &parent).expect("relation declared");
        let StressRelation::MovesBy { expected, .. } = declared.relation else {
            panic!("a calibrated procedure must be given an equivariance, not a tolerance");
        };

        let before = parent.prevalence();
        let predicted = (target / (1.0 - target)).ln() - (before / (1.0 - before)).ln();
        assert!((expected - predicted).abs() < 1e-12);

        let observed = scalar(&procedure, &stressed) - scalar(&procedure, &parent);
        assert!(
            (observed - expected).abs() < 1e-9,
            "expected a move of {expected}, observed {observed}"
        );
    }
}

#[test]
fn predictive_value_must_fall_when_the_base_rate_falls() {
    let parent = balanced();
    let procedure = Procedure::PositivePredictiveValue { threshold: 0.5 };
    let stress = full(
        "to-deployment",
        Knob::PrevalenceShift {
            target_prevalence: 0.05,
        },
    );
    let stressed = apply(&parent, &stress).expect("prevalence shift applies");
    let declared = declare(&stress, &procedure, &parent).expect("relation declared");
    assert_eq!(declared.obligation, Obligation::Required);

    let before = procedure.conclude(&parent).unwrap();
    let after = procedure.conclude(&stressed).unwrap();
    assert!(declared.relation.check(&before, &after).held());
    assert!(
        after.value.as_scalar().unwrap() < before.value.as_scalar().unwrap(),
        "predictive value must follow the base rate down"
    );
}

#[test]
fn a_group_contrast_is_a_within_class_quantity_and_survives_reweighting() {
    let parent = balanced();
    let stress = full(
        "to-deployment",
        Knob::PrevalenceShift {
            target_prevalence: 0.05,
        },
    );
    let stressed = apply(&parent, &stress).expect("prevalence shift applies");
    let before = scalar(&Procedure::GroupContrast, &parent);
    let after = scalar(&Procedure::GroupContrast, &stressed);
    assert!((before - after).abs() < 1e-12);
}

#[test]
fn reweighting_pays_for_the_shift_in_effective_sample_size_not_in_subjects() {
    let parent = balanced();
    let stress = full(
        "to-deployment",
        Knob::PrevalenceShift {
            target_prevalence: 0.02,
        },
    );
    let stressed = apply(&parent, &stress).expect("prevalence shift applies");

    assert_eq!(stressed.len(), parent.len());
    assert!(
        stressed.effective_n() < parent.effective_n(),
        "a reweighted cohort carries less information than its nominal size"
    );
    assert!(stressed.effective_n() < stressed.len() as f64);
}

#[test]
fn a_prevalence_target_of_zero_or_one_is_refused_rather_than_approximated() {
    let parent = balanced();
    for target in [0.0, 1.0, -0.1, 1.5] {
        let stress = full(
            "degenerate",
            Knob::PrevalenceShift {
                target_prevalence: target,
            },
        );
        assert!(matches!(
            apply(&parent, &stress),
            Err(StressError::PrevalenceOutOfRange { .. })
        ));
    }
}

#[test]
fn a_batch_offset_reaches_only_the_batch_it_names() {
    let parent = balanced();
    let stress = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
    );
    let outcome = perturb(&parent, &stress).expect("batch effect applies");
    assert!(outcome.is_valid(), "{:?}", outcome.defects());

    let expected = parent.pooled_within_sd().unwrap();
    for (before, after) in parent.subjects.iter().zip(outcome.cohort().subjects.iter()) {
        let moved = after.marker - before.marker;
        let target = if before.batch == "site-b" { expected } else { 0.0 };
        assert!((moved - target).abs() < 1e-9, "{} moved {moved}", before.id);
    }
}

#[test]
fn a_conclusion_that_flips_under_a_batch_effect_was_reading_the_batch() {
    let stress = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
    );

    let enriched = batch_enriched();
    let shifted = apply(&enriched, &stress).expect("batch effect applies");
    let declared = declare(&stress, &Procedure::GroupContrast, &enriched).unwrap();
    assert_eq!(declared.obligation, Obligation::Probed);
    let before = Procedure::GroupContrast.conclude(&enriched).unwrap();
    let after = Procedure::GroupContrast.conclude(&shifted).unwrap();
    assert!(
        !declared.relation.check(&before, &after).held(),
        "an effect size measured mostly on one site must move when that site moves"
    );
}

#[test]
fn a_conclusion_that_survives_a_batch_effect_was_reading_the_biology() {
    let parent = balanced();
    let stress = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
    );
    let shifted = apply(&parent, &stress).expect("batch effect applies");
    let declared = declare(&stress, &Procedure::GroupContrast, &parent).unwrap();
    let before = Procedure::GroupContrast.conclude(&parent).unwrap();
    let after = Procedure::GroupContrast.conclude(&shifted).unwrap();
    assert!(
        declared.relation.check(&before, &after).held(),
        "a site balanced across classes shifts both class means equally and cancels"
    );
}

#[test]
fn a_geometric_conclusion_is_untouched_by_a_marker_offset() {
    let parent = balanced();
    let stress = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 2.0,
        },
    );
    let shifted = apply(&parent, &stress).expect("batch effect applies");
    let procedure = Procedure::VolumeThreshold { mm3: 1_000.0 };
    let declared = declare(&stress, &procedure, &parent).unwrap();
    assert_eq!(declared.obligation, Obligation::Required);
    assert!(declared
        .relation
        .check(
            &procedure.conclude(&parent).unwrap(),
            &procedure.conclude(&shifted).unwrap()
        )
        .held());
}

#[test]
fn an_absent_batch_is_named_rather_than_silently_skipped() {
    let parent = balanced();
    let stress = full(
        "nowhere",
        Knob::BatchEffect {
            batch: "site-z".into(),
            offset_sd: 1.0,
        },
    );
    match apply(&parent, &stress) {
        Err(StressError::BatchAbsent { batch, .. }) => assert_eq!(batch, "site-z"),
        other => panic!("expected a named absent batch, got {other:?}"),
    }
}

#[test]
fn widening_the_assay_scales_dispersion_by_exactly_the_declared_factor() {
    let parent = balanced();
    for multiplier in [1.5, 2.0, 3.0] {
        let stress = full(
            "precision-loss",
            Knob::AssayDegradation {
                sd_multiplier: multiplier,
                limit_of_detection: None,
            },
        );
        let outcome = perturb(&parent, &stress).expect("assay degradation applies");
        assert!(outcome.is_valid(), "{:?}", outcome.defects());

        for condition in [true, false] {
            let before = parent.class_sd(condition).unwrap();
            let after = outcome.cohort().class_sd(condition).unwrap();
            assert!(
                (after - before * multiplier).abs() < 1e-9,
                "spread went {before} -> {after}, expected x{multiplier}"
            );
        }
    }
}

#[test]
fn widening_the_assay_does_not_move_either_class_mean() {
    let parent = balanced();
    let stress = full(
        "precision-loss",
        Knob::AssayDegradation {
            sd_multiplier: 4.0,
            limit_of_detection: None,
        },
    );
    let stressed = apply(&parent, &stress).expect("assay degradation applies");
    for condition in [true, false] {
        let before = parent.class_mean(condition).unwrap();
        let after = stressed.class_mean(condition).unwrap();
        assert!(
            (before - after).abs() < 1e-9,
            "class mean moved from {before} to {after}, so any downstream change is unattributable"
        );
    }
}

#[test]
fn a_measurement_below_the_limit_of_detection_becomes_unresolved_not_negative() {
    let parent = balanced();
    let stress = full(
        "precision-loss",
        Knob::AssayDegradation {
            sd_multiplier: 3.0,
            limit_of_detection: Some(-2.0),
        },
    );
    let outcome = perturb(&parent, &stress).expect("assay degradation applies");
    assert!(outcome.is_valid(), "{:?}", outcome.defects());
    assert!(
        outcome.cohort().unresolved_count() > 0,
        "a threefold loss of precision against this limit must censor somebody"
    );
    for subject in &outcome.cohort().subjects {
        assert_eq!(subject.resolved, subject.marker >= -2.0);
    }
    assert!(outcome
        .cohort()
        .resolved()
        .all(|subject| subject.marker >= -2.0));
}

#[test]
fn a_multiplier_below_one_is_refused_because_a_stress_never_narrows_uncertainty() {
    let parent = balanced();
    let stress = full(
        "impossible",
        Knob::AssayDegradation {
            sd_multiplier: 0.5,
            limit_of_detection: None,
        },
    );
    assert!(matches!(
        apply(&parent, &stress),
        Err(StressError::NarrowingMultiplier { .. })
    ));
}

#[test]
fn no_volume_moves_further_than_the_segmentation_reproducibility_it_declared() {
    let parent = balanced();
    for cv in [0.01, 0.05, 0.2] {
        let stress = full(
            "jitter",
            Knob::SegmentationJitter {
                reproducibility_cv: cv,
            },
        );
        let outcome = perturb(&parent, &stress).expect("jitter applies");
        assert!(outcome.is_valid(), "{:?}", outcome.defects());
        for (before, after) in parent.subjects.iter().zip(outcome.cohort().subjects.iter()) {
            let relative = (after.volume_mm3 - before.volume_mm3).abs() / before.volume_mm3;
            assert!(
                relative <= cv + 1e-12,
                "{} moved {relative} against a stated reproducibility of {cv}",
                before.id
            );
        }
    }
}

#[test]
fn volumes_separated_by_more_than_the_jitter_band_never_swap_places() {
    let parent = balanced();
    let stress = full(
        "jitter",
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    );
    let jittered = apply(&parent, &stress).expect("jitter applies");
    let declared = declare(&stress, &Procedure::VolumeRanking, &parent).unwrap();
    assert_eq!(declared.obligation, Obligation::Required);
    assert!(matches!(
        declared.relation,
        StressRelation::OrderPreservedBeyondRatio { .. }
    ));
    let outcome = declared.relation.check(
        &Procedure::VolumeRanking.conclude(&parent).unwrap(),
        &Procedure::VolumeRanking.conclude(&jittered).unwrap(),
    );
    assert!(outcome.held(), "{outcome:?}");
}

#[test]
fn volume_ordering_within_the_jitter_band_is_not_claimed_to_be_stable() {
    let parent = balanced();
    let stress = full(
        "jitter",
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    );
    let jittered = apply(&parent, &stress).expect("jitter applies");
    let before = Procedure::VolumeRanking.conclude(&parent).unwrap();
    let after = Procedure::VolumeRanking.conclude(&jittered).unwrap();
    assert!(
        !StressRelation::OrderPreserved.check(&before, &after).held(),
        "near-equal volumes do reorder under jitter; a relation that claimed otherwise would be \
         asserting more than the assay supports"
    );
}

#[test]
fn jitter_reaches_volumes_and_nothing_else() {
    let parent = balanced();
    let stress = full(
        "jitter",
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    );
    let jittered = apply(&parent, &stress).expect("jitter applies");
    for (before, after) in parent.subjects.iter().zip(jittered.subjects.iter()) {
        assert_eq!(before.marker, after.marker);
        assert_eq!(before.weight, after.weight);
        assert_eq!(before.batch, after.batch);
        assert_eq!(before.condition, after.condition);
        assert_ne!(before.volume_mm3, after.volume_mm3);
    }
}

#[test]
fn a_threshold_sitting_on_a_volume_reclassifies_inside_the_reproducibility_band() {
    let mut parent = Cohort::new(
        "cohort-on-threshold",
        (0..12)
            .map(|index| {
                let mut subject = Subject::new(
                    format!("SUBJ-{index:04}"),
                    if index % 2 == 0 { "site-a" } else { "site-b" },
                    index < 6,
                    index as f64 * 0.1,
                    1_000.0,
                );
                subject.volume_mm3 += index as f64 * 0.01;
                subject
            })
            .collect(),
    );
    parent.subjects[0].volume_mm3 = 1_000.0;
    let stress = full(
        "jitter",
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    );
    let jittered = apply(&parent, &stress).expect("jitter applies");
    let procedure = Procedure::VolumeThreshold { mm3: 1_000.0 };
    let before = procedure.conclude(&parent).unwrap();
    let after = procedure.conclude(&jittered).unwrap();
    assert_ne!(
        before.value.ids(),
        after.value.ids(),
        "a threshold laid on top of the volumes must reclassify somebody under jitter"
    );
    assert_eq!(
        declare(&stress, &procedure, &parent).unwrap().obligation,
        Obligation::Probed,
        "reclassification here is the finding, not a defect"
    );
}

#[test]
fn a_cohort_with_only_one_class_is_refused_rather_than_producing_a_nan() {
    let parent = Cohort::new(
        "cohort-one-class",
        (0..4)
            .map(|index| {
                Subject::new(
                    format!("SUBJ-{index:04}"),
                    "site-a",
                    true,
                    index as f64,
                    1_000.0,
                )
            })
            .collect(),
    );
    assert!(matches!(
        parent.validate(),
        Err(StressError::ClassAbsent { .. })
    ));
}

#[test]
fn a_conclusion_carries_the_number_of_subjects_the_assay_could_not_measure() {
    let parent = balanced();
    let stress = full(
        "precision-loss",
        Knob::AssayDegradation {
            sd_multiplier: 3.0,
            limit_of_detection: Some(-2.0),
        },
    );
    let stressed = apply(&parent, &stress).expect("assay degradation applies");
    let conclusion = Procedure::MarkerSeparation.conclude(&stressed).unwrap();
    assert_eq!(conclusion.unresolved, stressed.unresolved_count());
    assert!(conclusion.unresolved > 0);
    assert!(matches!(conclusion.value, ConclusionValue::Scalar(_)));
}
