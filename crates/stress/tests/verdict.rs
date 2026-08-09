//! The verdict, the coverage claim, and determinism.
//!
//! These are the tests that would have caught the audit finding this crate exists to close, and
//! the ones that stop the verdict quietly degrading into a score.

use bioprism_stress::{
    coverage, declare, profile, standard_panel, standard_program, Cohort, FamilyOwner, Knob,
    Magnitude, Obligation, Procedure, ReferenceFamily, RobustnessProfile, Stress, StressFamily,
    StressReport, Subject,
};

const SEED: u64 = 20_260_808;

fn balanced() -> Cohort {
    Cohort::synthetic("cohort-balanced", SEED, 40, 0.4, 1.5)
}

fn full(id: &str, knob: Knob) -> Stress {
    Stress::new(id, knob, Magnitude::FULL, SEED)
}

fn run(knob: Knob) -> RobustnessProfile {
    profile(&balanced(), &full("stress", knob), &standard_panel())
        .expect("the standard panel is defined on this cohort")
}

#[test]
fn a_profile_reports_the_intensity_at_which_a_conclusion_broke_not_a_pass_mark() {
    let outcome = run(Knob::BatchEffect {
        batch: "site-b".into(),
        offset_sd: 2.0,
    });
    let broken = outcome.first_to_break().expect("a two-sigma site offset breaks something");
    let magnitude = broken.broke_at.expect("a break carries its intensity");
    assert!(magnitude > Magnitude::ZERO && magnitude <= Magnitude::FULL);
    assert!(broken.observed_at_break.is_some());
    assert!(broken.expected_at_break.is_some());
    assert!(outcome.headline().contains("Breaking points"));
}

#[test]
fn a_conclusion_that_holds_to_full_magnitude_is_reported_as_held_through_not_as_passing() {
    let outcome = run(Knob::PrevalenceShift {
        target_prevalence: 0.02,
    });
    let ranking = outcome
        .findings
        .iter()
        .find(|finding| finding.conclusion_id == "marker_ranking")
        .expect("the panel ranks subjects");
    assert!(ranking.survived());
    assert_eq!(ranking.held_through, Some(Magnitude::FULL));
    assert!(ranking.line().contains("held through"));
}

#[test]
fn a_generator_whose_postcondition_fails_abandons_the_rung_instead_of_scoring_it() {
    let parent = Cohort::synthetic("cohort-tiny", SEED, 4, 0.5, 1.5);
    let outcome = profile(
        &parent,
        &full(
            "precision-loss",
            Knob::AssayDegradation {
                sd_multiplier: 3.0,
                limit_of_detection: None,
            },
        ),
        &standard_panel(),
    )
    .expect("the profile still runs");

    assert!(
        !outcome.generator_is_sound(),
        "two subjects per class leave no direction for orthogonal noise, so the declared \
         dispersion cannot be reached"
    );
    assert!(outcome.sweep.iter().all(|point| point.abandoned));
    assert!(
        outcome.findings.iter().all(|finding| finding.broke_at.is_none()
            && finding.held_through.is_none()),
        "nothing may be scored at an intensity the generator did not deliver"
    );
    assert!(outcome.headline().contains("GENERATOR DEFECTIVE"));
}

#[test]
fn a_batch_shift_on_a_single_class_batch_is_reported_as_non_identifiable_not_as_a_pass() {
    let parent = Cohort::synthetic_confounded("cohort-confounded", SEED, 40, 0.4, 1.5);
    let outcome = profile(
        &parent,
        &full(
            "site-shift",
            Knob::BatchEffect {
                batch: "site-b".into(),
                offset_sd: 1.0,
            },
        ),
        &standard_panel(),
    )
    .expect("the profile still runs");

    assert!(!outcome.identifiability.informative());
    assert!(outcome.headline().contains("NOT IDENTIFIABLE"));
    assert!(!outcome.headline().contains("held to full magnitude"));
}

#[test]
fn a_batch_shift_on_a_mixed_batch_is_identifiable_and_says_by_how_much() {
    let outcome = run(Knob::BatchEffect {
        batch: "site-b".into(),
        offset_sd: 1.0,
    });
    match outcome.identifiability {
        bioprism_stress::Identifiability::Separable { overlap, .. } => assert!(overlap > 0.0),
        other => panic!("a balanced cohort must be separable, got {other:?}"),
    }
}

#[test]
fn the_worst_family_is_the_one_that_broke_a_conclusion_at_the_lowest_intensity() {
    let parent = balanced();
    let report = StressReport::run(&parent, &standard_program("site-b"), &standard_panel())
        .expect("the standard program runs");
    let worst = report.worst_family().expect("something breaks somewhere");
    let worst_at = worst.first_to_break().and_then(|f| f.broke_at).unwrap();

    for other in &report.profiles {
        if let Some(magnitude) = other.first_to_break().and_then(|f| f.broke_at) {
            assert!(
                worst_at <= magnitude,
                "{} broke at {magnitude}, before the reported worst family at {worst_at}",
                other.family.as_str()
            );
        }
    }
}

#[test]
fn every_stress_in_the_standard_program_satisfies_its_own_postconditions() {
    let parent = balanced();
    let report = StressReport::run(&parent, &standard_program("site-b"), &standard_panel())
        .expect("the standard program runs");
    assert_eq!(report.profiles.len(), 4);
    for outcome in &report.profiles {
        assert!(
            outcome.generator_is_sound(),
            "{} declared postconditions it did not meet: {:?}",
            outcome.family.as_str(),
            outcome.generator_defects
        );
    }
}

#[test]
fn a_required_relation_is_never_reported_as_a_robustness_finding() {
    let parent = balanced();
    let report = StressReport::run(&parent, &standard_program("site-b"), &standard_panel())
        .expect("the standard program runs");
    for outcome in &report.profiles {
        assert!(
            outcome.mislabelled_procedures().is_empty(),
            "{} broke a required relation: {:?}",
            outcome.family.as_str(),
            outcome
                .mislabelled_procedures()
                .iter()
                .map(|finding| finding.line())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn the_sweep_records_effective_sample_size_falling_as_the_base_rate_is_pushed_down() {
    let outcome = run(Knob::PrevalenceShift {
        target_prevalence: 0.02,
    });
    let effective: Vec<f64> = outcome.sweep.iter().map(|point| point.effective_n).collect();
    for window in effective.windows(2) {
        assert!(
            window[1] < window[0],
            "reweighting harder must cost more information, not less: {effective:?}"
        );
    }
    let last = outcome.sweep.last().unwrap();
    assert_eq!(last.nominal_n, 40);
    assert!(
        last.effective_n < last.nominal_n as f64,
        "the honest denominator is the effective size, not the head count"
    );
}

#[test]
fn censoring_is_reported_as_a_drift_in_the_analysable_base_rate() {
    let parent = balanced();
    let outcome = profile(
        &parent,
        &full(
            "precision-loss",
            Knob::AssayDegradation {
                sd_multiplier: 3.0,
                limit_of_detection: Some(-2.0),
            },
        ),
        &standard_panel(),
    )
    .expect("the profile runs");

    let last = outcome.sweep.last().unwrap();
    assert!(last.unresolved > 0);
    assert!(
        (last.analysable_prevalence - parent.prevalence()).abs() > 1e-9,
        "class-dependent censoring moves the base rate over the subjects that remain, and the \
         sweep has to say so"
    );
}

#[test]
fn every_mutation_family_named_by_38_01_has_a_generator() {
    let table = coverage();
    assert_eq!(table.len(), 6);
    for row in &table {
        match &row.owner {
            FamilyOwner::LeakageMutation { mechanism } => assert!(!mechanism.is_empty()),
            FamilyOwner::BiologicalStress { family } => {
                assert!(StressFamily::ALL.contains(family));
            }
            FamilyOwner::Shared { mechanism, family } => {
                assert!(!mechanism.is_empty());
                assert!(StressFamily::ALL.contains(family));
            }
        }
    }
}

#[test]
fn the_three_families_the_platform_audit_found_missing_are_owned_by_this_crate() {
    for (family, expected) in [
        (
            ReferenceFamily::PrevalenceShift,
            StressFamily::PrevalenceShift,
        ),
        (
            ReferenceFamily::SegmentationPerturbation,
            StressFamily::SegmentationJitter,
        ),
        (
            ReferenceFamily::AssayUncertainty,
            StressFamily::AssayDegradation,
        ),
    ] {
        assert_eq!(
            family.owner(),
            FamilyOwner::BiologicalStress { family: expected },
            "{} must have a generator here",
            family.blueprint_wording()
        );
    }
}

#[test]
fn a_family_implemented_by_two_mechanisms_says_so_rather_than_claiming_one() {
    assert!(matches!(
        ReferenceFamily::SiteStyleSwap.owner(),
        FamilyOwner::Shared { .. }
    ));
    for family in [
        ReferenceFamily::WrongTimeSpecimenPairing,
        ReferenceFamily::DuplicateParticipantAcrossSplits,
    ] {
        assert_eq!(family.stress_family(), None, "leakage stays with mutation");
    }
}

#[test]
fn every_stress_family_covers_a_family_the_reference_world_names() {
    for family in StressFamily::ALL {
        assert!(
            ReferenceFamily::ALL
                .into_iter()
                .any(|reference| reference.stress_family() == Some(family)),
            "{} implements nothing 38.01 asked for",
            family.as_str()
        );
    }
}

#[test]
fn the_same_seed_produces_the_same_stressed_cohort_byte_for_byte() {
    let parent = balanced();
    for knob in [
        Knob::PrevalenceShift {
            target_prevalence: 0.05,
        },
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
        Knob::AssayDegradation {
            sd_multiplier: 3.0,
            limit_of_detection: Some(-2.0),
        },
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    ] {
        let stress = full("repeat", knob);
        let first = bioprism_stress::apply(&parent, &stress).unwrap();
        let second = bioprism_stress::apply(&parent, &stress).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}

#[test]
fn a_different_seed_produces_a_different_stressed_cohort() {
    let parent = balanced();
    let knob = Knob::SegmentationJitter {
        reproducibility_cv: 0.05,
    };
    let first = bioprism_stress::apply(
        &parent,
        &Stress::new("jitter", knob.clone(), Magnitude::FULL, 1),
    )
    .unwrap();
    let second = bioprism_stress::apply(
        &parent,
        &Stress::new("jitter", knob, Magnitude::FULL, 2),
    )
    .unwrap();
    assert_ne!(first.digest().unwrap(), second.digest().unwrap());
}

#[test]
fn a_cohort_digest_ignores_the_label_and_reads_the_data() {
    let mut renamed = balanced();
    renamed.id = "a-completely-different-name".into();
    assert_eq!(balanced().digest().unwrap(), renamed.digest().unwrap());

    let mut edited = balanced();
    edited.subjects[0].marker += 1e-9;
    assert_ne!(balanced().digest().unwrap(), edited.digest().unwrap());
}

#[test]
fn a_robustness_profile_round_trips_through_json() {
    let outcome = run(Knob::BatchEffect {
        batch: "site-b".into(),
        offset_sd: 1.0,
    });
    let encoded = serde_json::to_string(&outcome).expect("a profile serialises");
    let decoded: RobustnessProfile = serde_json::from_str(&encoded).expect("and comes back");
    assert_eq!(outcome, decoded);
}

#[test]
fn a_discriminative_invariance_is_required_while_a_batch_probe_is_only_hypothesised() {
    let parent = balanced();
    let shift = full(
        "shift",
        Knob::PrevalenceShift {
            target_prevalence: 0.05,
        },
    );
    let batch = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
    );
    assert_eq!(
        declare(&shift, &Procedure::MarkerSeparation, &parent)
            .unwrap()
            .obligation,
        Obligation::Required
    );
    assert_eq!(
        declare(&batch, &Procedure::MarkerSeparation, &parent)
            .unwrap()
            .obligation,
        Obligation::Probed
    );
}

#[test]
fn every_family_and_procedure_pair_declares_a_relation_with_a_stated_reason() {
    let parent = balanced();
    for stress in standard_program("site-b") {
        for procedure in standard_panel() {
            let declared = declare(&stress, &procedure, &parent).unwrap_or_else(|error| {
                panic!(
                    "{} has no declared relation for {}: {error}",
                    stress.family().as_str(),
                    procedure.id()
                )
            });
            assert!(
                declared.rationale.len() > 30,
                "{} against {} declares a relation with no stated reason",
                stress.family().as_str(),
                procedure.id()
            );
        }
    }
}

#[test]
fn a_profile_states_that_its_breaking_points_are_bounded_by_the_ladder_resolution() {
    let outcome = run(Knob::SegmentationJitter {
        reproducibility_cv: 0.05,
    });
    assert!(outcome.caveat.contains("upper bounds"));
    assert_eq!(outcome.sweep.len(), Magnitude::ladder().len());
    assert_eq!(outcome.blueprint_module, "30.05");
}

#[test]
fn a_stress_family_names_the_blueprint_module_it_answers_to() {
    for family in StressFamily::ALL {
        assert!(family.blueprint_module().starts_with("3"));
        assert!(family.claim().len() > 40);
    }
}

#[test]
fn a_subject_the_assay_lost_is_excluded_from_every_statistic_it_would_have_biased() {
    let mut parent = balanced();
    parent.subjects[0].resolved = false;
    let counted = parent.resolved().count();
    assert_eq!(counted, parent.len() - 1);
    assert!(!parent
        .ranking()
        .iter()
        .any(|ranked| ranked.id == parent.subjects[0].id));
    assert_eq!(parent.unresolved_count(), 1);
}

#[test]
fn a_stress_at_a_lower_magnitude_is_the_same_stress() {
    let stress = full(
        "site-shift",
        Knob::BatchEffect {
            batch: "site-b".into(),
            offset_sd: 1.0,
        },
    );
    let quarter = stress.at(Magnitude::from_permille(250));
    assert_eq!(quarter.knob, stress.knob);
    assert_eq!(quarter.seed, stress.seed);
    assert_eq!(quarter.family(), StressFamily::BatchEffect);
    assert!(quarter.id.contains("0.250"));
}

#[test]
fn a_broken_ordering_names_the_rank_that_moved_rather_than_dumping_the_list() {
    let outcome = run(Knob::BatchEffect {
        batch: "site-b".into(),
        offset_sd: 2.0,
    });
    let ranking = outcome
        .findings
        .iter()
        .find(|finding| finding.conclusion_id == "marker_ranking")
        .expect("the panel ranks subjects");
    let observed = ranking
        .observed_at_break
        .as_deref()
        .expect("a two-sigma offset reorders a forty-subject cohort");
    assert!(observed.contains("at rank"), "{observed}");
    assert!(
        observed.len() < 60,
        "a violation that prints the whole ordering buries the subject that moved: {observed}"
    );
}

#[test]
fn a_magnitude_beyond_the_declared_stress_is_unrepresentable() {
    assert_eq!(Magnitude::from_permille(9_999), Magnitude::FULL);
    assert!(serde_json::from_str::<Magnitude>("1000").is_ok());
    assert!(
        serde_json::from_str::<Magnitude>("1001").is_err(),
        "a magnitude past the knob its postconditions were written against must not deserialise"
    );
    assert_eq!(serde_json::to_string(&Magnitude::FULL).unwrap(), "1000");
}

#[test]
fn every_postcondition_a_stress_declares_is_actually_executed_against_it() {
    let parent = balanced();
    for stress in standard_program("site-b") {
        let declared = bioprism_stress::postconditions(&parent, &stress)
            .expect("the stress declares its postconditions");
        let outcome = bioprism_stress::perturb(&parent, &stress).expect("and applies");
        assert!(!declared.is_empty(), "a stress with no postcondition is not a test");
        assert_eq!(
            declared.len(),
            outcome.checks().len(),
            "{} declared {} postconditions but ran {}",
            stress.family().as_str(),
            declared.len(),
            outcome.checks().len()
        );
        for (invariant, check) in declared.iter().zip(outcome.checks()) {
            assert_eq!(&check.invariant, invariant);
        }
    }
}

#[test]
fn a_stressed_cohort_arrives_with_the_evidence_that_it_was_checked() {
    let parent = balanced();
    let stress = full(
        "jitter",
        Knob::SegmentationJitter {
            reproducibility_cv: 0.05,
        },
    );
    let outcome = bioprism_stress::perturb(&parent, &stress).expect("jitter applies");
    assert!(outcome.is_valid());
    assert_eq!(outcome.stress().id, stress.id);
    let digest = outcome.cohort().digest().unwrap();
    assert_eq!(outcome.into_cohort().digest().unwrap(), digest);
}

#[test]
fn a_cohort_that_repeats_a_subject_is_refused_because_repeats_inflate_the_sample() {
    let parent = Cohort::new(
        "cohort-duplicated",
        vec![
            Subject::new("SUBJ-0000", "site-a", true, 1.0, 1_000.0),
            Subject::new("SUBJ-0000", "site-a", false, 0.0, 1_000.0),
        ],
    );
    assert!(matches!(
        parent.validate(),
        Err(bioprism_stress::StressError::DuplicateSubject { .. })
    ));
}
