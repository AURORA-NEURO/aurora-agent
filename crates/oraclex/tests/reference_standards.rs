//! Section 31: what a reference standard licenses, and what it refuses to.

use bioprism_ids::ContentHash;
use bioprism_oracle::{EvidenceTier, UtcTimestamp};
use bioprism_oraclex::audit::{
    independent_reproduction, publication_integrity, separation, unilateral_control, ReportedFinding,
    Role, RoleAssignment, INCOMPATIBLE,
};
use bioprism_oraclex::endpoint::{
    comparable, override_witnesses, reconcile, Assessment, DateClaim, Eligibility, FollowUp, Outcome,
    ResponseCriteria, SourceHierarchy,
};
use bioprism_oraclex::identity::{
    decide, Concordance, IdentityClaim, IdentitySignal, Lineage, Mixture, SpecimenRef,
};
use bioprism_oraclex::longitudinal::{
    revision_gain, score_forecast, Ascertainment, Escrow, Forecast, RevealRule, Snapshot,
};
use bioprism_oraclex::orthogonal::{
    are_orthogonal, confirm, Direction, Excluded, Expectation, Explanation, Modality, Observation,
    SharedFailureMode,
};
use bioprism_oraclex::panel::{
    Adjudication, Blinding, ConsensusRule, Read, ReaderPanel,
};
use bioprism_oraclex::perturbation::{
    controls_complete, decide as perturbation_decide, positivity, ClaimPlane, ControlKind,
    EvidenceSource, PerturbationEvidence, Reagent, Rescue, Stratum,
};
use bioprism_oraclex::standard::{
    ClassCall, ReferenceBasis, ReferenceLevel, ReferenceStandard, SourceObservation, StandardHistory,
};
use bioprism_oraclex::OracleXError;
use bioprism_scope::ScopeKey;
use std::collections::BTreeMap;

fn at(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("fixed test timestamp")
}

fn population() -> ScopeKey {
    ScopeKey::new().exact("cohort", "reference")
}

// ---- 31.05 identity and lineage ------------------------------------------------------------

#[test]
fn molecular_identity_evidence_overrides_a_textual_join() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P1", "A2", "T0");
    let signals = [
        IdentitySignal::TextualCrosswalk {
            join_key: "truncated_barcode".into(),
            agrees: true,
        },
        IdentitySignal::GenotypeFingerprint {
            concordance: Concordance::Discordant,
        },
    ];

    let determination = decide(IdentityClaim::SameSubject, &left, &right, &signals);

    assert_eq!(determination.tier(), Some(EvidenceTier::Deterministic));
    assert_eq!(determination.witnesses().len(), 1);
    assert!(
        !determination.is_supported(),
        "31.05's worked case: identity evidence overrides the textual join"
    );
}

#[test]
fn the_override_comes_from_the_shared_ladder_and_is_not_re_derived_here() {
    let molecular = IdentitySignal::GenotypeFingerprint {
        concordance: Concordance::Discordant,
    }
    .tier();
    let textual = IdentitySignal::TextualCrosswalk {
        join_key: "id".into(),
        agrees: true,
    }
    .tier();

    assert!(molecular.may_override(textual));
    assert!(!textual.may_override(molecular));
    assert!(
        molecular.may_override(molecular),
        "equal tiers return true: same-tier conflict is a disagreement, not an override"
    );
}

#[test]
fn a_mixture_contradicts_single_source_and_leaves_same_subject_unresolved() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P1", "A2", "T0");
    let signals = [IdentitySignal::Mixture {
        mixture: Mixture::new(2).with_minor_fraction_permille(150),
    }];

    let single = decide(IdentityClaim::SingleSource, &left, &right, &signals);
    let same = decide(IdentityClaim::SameSubject, &left, &right, &signals);

    assert_eq!(single.witnesses().len(), 1);
    assert!(!same.decided());
    assert!(same
        .missing()
        .iter()
        .any(|gap| gap.evidence.contains("deconvolved")));
}

#[test]
fn concordant_sex_chromosomes_do_not_confirm_identity() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P2", "A2", "T0");
    let signals = [IdentitySignal::SexChromosome {
        concordance: Concordance::Concordant,
    }];

    let determination = decide(IdentityClaim::SameSubject, &left, &right, &signals);

    assert!(
        !determination.is_supported(),
        "two unrelated subjects of one sex are concordant; the signal falsifies and does not confirm"
    );
}

#[test]
fn no_identity_signal_at_all_is_not_evaluable_rather_than_supported() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P1", "A2", "T0");
    let determination = decide(IdentityClaim::SameSubject, &left, &right, &[]);
    assert!(matches!(
        determination,
        bioprism_oraclex::Determination::NotEvaluable(_)
    ));
}

#[test]
fn a_derivation_cycle_is_a_deterministic_contradiction() {
    let a = SpecimenRef::new("P1", "A", "T0");
    let b = SpecimenRef::new("P1", "B", "T0");
    let lineage = Lineage::new()
        .derive(a.clone(), b.clone())
        .derive(b, a);

    let determination = lineage.acyclic();

    assert_eq!(determination.tier(), Some(EvidenceTier::Deterministic));
    assert_eq!(determination.witnesses().len(), 1);
}

#[test]
fn samples_from_one_participant_are_reported_as_a_group_a_random_split_would_break() {
    let lineage = Lineage::new()
        .declare(SpecimenRef::new("P1", "A", "T0"))
        .declare(SpecimenRef::new("P1", "B", "T1"))
        .declare(SpecimenRef::new("P2", "C", "T0"));

    let groups = lineage.participant_groups();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups["P1"].len(), 2);
}

// ---- 31.06 / 32.18 reader panels -----------------------------------------------------------

#[test]
fn changing_the_consensus_rule_changes_the_reference_and_never_erases_the_reads() {
    let panel = ReaderPanel::new([
        Read::independent("r1", "progression"),
        Read::independent("r2", "progression"),
        Read::independent("r3", "treatment effect"),
    ])
    .expect("distinct readers");

    let majority = panel.reference(ConsensusRule::Majority).expect("non-empty");
    let unanimous = panel.reference(ConsensusRule::Unanimous).expect("non-empty");

    assert!(majority.consensus().is_supported());
    assert!(!unanimous.consensus().is_supported());
    assert_eq!(
        majority.reads(),
        unanimous.reads(),
        "32.18: the rule changes the reference distribution and not the original reads"
    );
}

#[test]
fn a_split_panel_is_unresolved_rather_than_the_first_listed_call() {
    let panel = ReaderPanel::new([
        Read::independent("r1", "alpha"),
        Read::independent("r2", "beta"),
    ])
    .expect("distinct readers");

    let determination = panel
        .reference(ConsensusRule::Majority)
        .expect("non-empty")
        .consensus();

    assert!(!determination.decided());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.because.contains("split")));
}

#[test]
fn a_minority_call_survives_a_rule_that_does_not_select_it() {
    let panel = ReaderPanel::new([
        Read::independent("r1", "alpha"),
        Read::independent("r2", "alpha"),
        Read::independent("r3", "beta").citing(["a cited region"]),
    ])
    .expect("distinct readers");

    let reference = panel.reference(ConsensusRule::Majority).expect("non-empty");

    assert!(reference.consensus().is_supported());
    assert!(reference.minority_calls().contains("beta"));
    assert!(reference.reads().iter().any(|read| read.call == "beta"));
}

#[test]
fn an_unblinded_adjudication_is_not_a_reference() {
    let panel = ReaderPanel::new([
        Read::independent("r1", "alpha"),
        Read::independent("r2", "beta"),
    ])
    .expect("distinct readers")
    .with_adjudication(Adjudication::new("chair", "alpha", Blinding::unblinded()));

    let determination = panel
        .reference(ConsensusRule::Adjudicated)
        .expect("non-empty")
        .consensus();

    assert!(!determination.is_supported());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.evidence.contains("blinded")));
}

#[test]
fn a_duplicate_reader_is_rejected_because_the_panel_size_would_overstate_independence() {
    let result = ReaderPanel::new([
        Read::independent("r1", "alpha"),
        Read::independent("r1", "alpha"),
    ]);
    assert_eq!(
        result,
        Err(OracleXError::DuplicateReader {
            reader: "r1".to_string()
        })
    );
}

#[test]
fn post_discussion_reads_do_not_count_toward_a_consensus_rule() {
    let panel = ReaderPanel::new([
        Read::independent("r1", "alpha"),
        Read::post_discussion("r1", "beta"),
        Read::post_discussion("r2", "beta"),
    ])
    .expect("one read per reader per phase");

    let reference = panel.reference(ConsensusRule::Unanimous).expect("non-empty");

    assert_eq!(reference.readers(), 1);
    assert!(reference.consensus().is_supported());
    assert_eq!(reference.reads().len(), 3, "the other reads are retained");
}

// ---- 31.07 / 32.13 orthogonal confirmation -------------------------------------------------

#[test]
fn two_channels_sharing_a_failure_mode_cannot_confirm_each_other() {
    let shared = [SharedFailureMode::Aliquot];
    let rna = Modality::new("rna", "S1", "T0").sharing(shared.clone());
    let protein = Modality::new("protein", "S1", "T0").sharing(shared);
    assert!(!are_orthogonal(&rna, &protein));

    let expectation = Expectation::new("target is up", Direction::Higher, "two-fold");
    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Higher),
        &Excluded::none(),
    );

    assert!(!determination.is_supported());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.because.contains("not confirmation")));
}

#[test]
fn independent_channels_moving_as_predicted_do_confirm() {
    let rna = Modality::new("rna", "S1", "T0");
    let protein = Modality::new("protein", "S1", "T0");
    let expectation = Expectation::new("target is up", Direction::Higher, "two-fold");

    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Higher),
        &Excluded::none(),
    );

    assert!(determination.is_supported());
    assert_eq!(determination.tier(), Some(EvidenceTier::Statistical));
}

#[test]
fn discordance_lists_the_explanations_that_remain_open() {
    let rna = Modality::new("rna", "S1", "T0");
    let protein = Modality::new("protein", "S1", "T0");
    let expectation = Expectation::new("target is up", Direction::Higher, "two-fold");

    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Lower),
        &Excluded::none(),
    );

    assert_eq!(
        determination.missing().len(),
        Explanation::ALL.len(),
        "31.07: disagreement is not automatically error; every candidate stays open"
    );
}

#[test]
fn discordance_with_every_explanation_ruled_out_is_a_contradiction() {
    let rna = Modality::new("rna", "S1", "T0");
    let protein = Modality::new("protein", "S1", "T0");
    let expectation = Expectation::new("target is up", Direction::Higher, "two-fold");
    let mut excluded = Excluded::none();
    for explanation in Explanation::ALL {
        excluded = excluded.rule_out(explanation, "a cited control experiment");
    }

    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Lower),
        &excluded,
    );

    assert_eq!(determination.witnesses().len(), 1);
}

#[test]
fn a_specimen_mismatch_reopens_identity_and_region_even_when_the_caller_excluded_them() {
    let rna = Modality::new("rna", "S1", "T0");
    let protein = Modality::new("protein", "S2", "T0");
    let expectation = Expectation::new("target is up", Direction::Higher, "two-fold");
    let mut excluded = Excluded::none();
    for explanation in Explanation::ALL {
        excluded = excluded.rule_out(explanation, "asserted without evidence");
    }

    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Lower),
        &excluded,
    );

    let named: Vec<String> = determination
        .missing()
        .iter()
        .map(|gap| gap.evidence.clone())
        .collect();
    assert!(named.iter().any(|gap| gap.contains("identity")));
    assert!(named.iter().any(|gap| gap.contains("region")));
}

#[test]
fn an_expectation_without_a_tolerance_makes_the_comparison_not_evaluable() {
    let rna = Modality::new("rna", "S1", "T0");
    let protein = Modality::new("protein", "S1", "T0");
    let expectation = Expectation::new("target is up", Direction::Higher, "  ");

    let determination = confirm(
        &expectation,
        &Observation::new(rna, Direction::Higher),
        &Observation::new(protein, Direction::Higher),
        &Excluded::none(),
    );

    assert!(matches!(
        determination,
        bioprism_oraclex::Determination::NotEvaluable(_)
    ));
}

// ---- 31.08 / 32.17 perturbation ------------------------------------------------------------

#[test]
fn observational_evidence_cannot_reach_an_interventional_plane() {
    let evidence = PerturbationEvidence::new("GENE1", EvidenceSource::Observational);

    assert!(perturbation_decide(ClaimPlane::Prediction, &evidence).is_supported());
    for plane in [ClaimPlane::Phenotype, ClaimPlane::Target, ClaimPlane::Mechanism] {
        assert!(matches!(
            perturbation_decide(plane, &evidence),
            bioprism_oraclex::Determination::NotEvaluable(_)
        ));
    }
}

#[test]
fn a_failed_rescue_with_a_dissenting_reagent_contradicts_the_mechanism_claim() {
    let evidence = PerturbationEvidence::new("GENE1", EvidenceSource::Interventional)
        .with_control(ControlKind::Positive)
        .with_control(ControlKind::Negative)
        .with_control(ControlKind::Vehicle)
        .with_reagent(Reagent::new("g1", "crispr"), true)
        .with_reagent(Reagent::new("g2", "crispr"), false)
        .with_rescue(Rescue::NotRescued);

    let determination = perturbation_decide(ClaimPlane::Mechanism, &evidence);

    assert_eq!(determination.witnesses().len(), 1);
    assert!(!determination.is_supported());
}

#[test]
fn an_untried_rescue_is_unresolved_and_not_a_failed_one() {
    let evidence = PerturbationEvidence::new("GENE1", EvidenceSource::Interventional)
        .with_control(ControlKind::Positive)
        .with_control(ControlKind::Negative)
        .with_control(ControlKind::Vehicle)
        .with_reagent(Reagent::new("g1", "crispr"), true)
        .with_reagent(Reagent::new("d1", "small_molecule"), true)
        .with_rescue(Rescue::NotAttempted);

    let determination = perturbation_decide(ClaimPlane::Mechanism, &evidence);

    assert!(!determination.decided());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.evidence.contains("rescue")));
}

#[test]
fn two_reagents_of_one_modality_leave_the_target_claim_unresolved() {
    let evidence = PerturbationEvidence::new("GENE1", EvidenceSource::Interventional)
        .with_control(ControlKind::Positive)
        .with_control(ControlKind::Negative)
        .with_control(ControlKind::Vehicle)
        .with_reagent(Reagent::new("g1", "crispr"), true)
        .with_reagent(Reagent::new("g2", "crispr"), true);

    let determination = perturbation_decide(ClaimPlane::Target, &evidence);

    assert!(!determination.decided());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.evidence.contains("second modality")));
}

#[test]
fn a_missing_plane_control_is_unresolved_but_an_unrun_declared_control_is_a_contradiction() {
    let missing_from_plane = PerturbationEvidence::new("GENE1", EvidenceSource::Interventional)
        .with_reagent(Reagent::new("g1", "crispr"), true);
    assert!(!perturbation_decide(ClaimPlane::Phenotype, &missing_from_plane).decided());

    let promised_and_skipped = PerturbationEvidence::new("GENE1", EvidenceSource::Interventional)
        .with_control(ControlKind::Vehicle)
        .with_declared_but_unrun_control(ControlKind::Negative);
    let determination = controls_complete(&promised_and_skipped);
    assert_eq!(determination.witnesses().len(), 1);
}

#[test]
fn a_stratum_with_an_empty_arm_makes_positivity_unresolved() {
    let determination = positivity(&[
        Stratum::new("young", 40, 60),
        Stratum::new("elderly", 0, 25),
    ]);

    assert!(!determination.decided());
    assert!(determination
        .missing()
        .iter()
        .any(|gap| gap.because.contains("elderly")));
}

// ---- 31.09 longitudinal --------------------------------------------------------------------

#[test]
fn a_forecast_citing_evidence_the_snapshot_lacks_is_contradicted_even_when_it_is_right() {
    let snapshot = Snapshot::freeze(at("2026-01-01T00:00:00Z"), ["baseline_mri"]);
    let escrow = Escrow::new(
        &snapshot,
        [RevealRule::new(
            "six-month",
            at("2025-12-01T00:00:00Z"),
            "six months of follow-up",
        )],
        "technical",
        Ascertainment::Complete,
    )
    .expect("rule predates the freeze");
    let token = escrow.fire("six-month", true).expect("condition met");

    let leaky = Forecast::new("technical", ["baseline_mri", "month_six_biopsy"]);
    let determination = score_forecast(&snapshot, &leaky, "technical", &token);

    assert_eq!(determination.witnesses().len(), 1);
    assert!(
        !determination.is_supported(),
        "a right answer from a basis that did not exist is not a pass"
    );
}

#[test]
fn a_reveal_rule_declared_after_the_freeze_is_rejected() {
    let snapshot = Snapshot::freeze(at("2026-01-01T00:00:00Z"), ["baseline_mri"]);
    let result = Escrow::new(
        &snapshot,
        [RevealRule::new(
            "written-after-the-fact",
            at("2026-06-01T00:00:00Z"),
            "whatever the outcome turned out to be",
        )],
        "progression",
        Ascertainment::Complete,
    );
    assert!(matches!(
        result,
        Err(OracleXError::RuleDeclaredAfterFreeze { .. })
    ));
}

#[test]
fn an_escrow_does_not_open_for_a_rule_that_was_never_declared() {
    let snapshot = Snapshot::freeze(at("2026-01-01T00:00:00Z"), ["baseline_mri"]);
    let escrow = Escrow::new(
        &snapshot,
        [RevealRule::new("six-month", at("2025-12-01T00:00:00Z"), "follow-up")],
        "progression",
        Ascertainment::Complete,
    )
    .expect("rule predates the freeze");

    assert!(matches!(
        escrow.fire("invented-on-the-spot", true),
        Err(OracleXError::EscrowSealed { .. })
    ));
    assert!(matches!(
        escrow.fire("six-month", false),
        Err(OracleXError::EscrowSealed { .. })
    ));
    let token = escrow.fire("six-month", true).expect("condition met");
    assert_eq!(escrow.open(&token).expect("token is valid"), &"progression");
}

#[test]
fn revision_gain_is_none_when_both_forecasts_were_equally_right_or_wrong() {
    let a = Forecast::new("progression", ["mri"]);
    let b = Forecast::new("progression", ["mri", "biopsy"]);
    assert_eq!(revision_gain(&a, &b, "progression"), None);

    let worse = Forecast::new("technical", ["mri", "biopsy"]);
    assert_eq!(revision_gain(&a, &worse, "progression"), Some(false));
}

// ---- 31.10 / 31.11 / 32.12 reference standards ---------------------------------------------

#[test]
fn a_standard_measured_at_one_level_abstains_about_the_others() {
    let standard = ReferenceStandard::new(
        "seg-v1",
        population(),
        ReferenceBasis::ReaderConsensus {
            rule: "majority".into(),
            readers: 3,
        },
        [SourceObservation::new("mri", "three reader masks")],
    )
    .expect("one source")
    .with_agreement(
        bioprism_oraclex::standard::LevelAgreement::new(
            ReferenceLevel::Boundary,
            "surface distance",
            1.5,
        )
        .expect("finite"),
    );

    assert!(standard.agreement_at(ReferenceLevel::Boundary).is_supported());
    for level in [
        ReferenceLevel::Detection,
        ReferenceLevel::Topology,
        ReferenceLevel::DownstreamUse,
    ] {
        assert!(
            !standard.agreement_at(level).is_supported(),
            "31.10: similar overlap does not answer a detection or downstream question"
        );
    }
}

#[test]
fn a_reader_consensus_cannot_be_promoted_above_the_statistical_tier() {
    let standard = ReferenceStandard::new(
        "path-v1",
        population(),
        ReferenceBasis::ReaderConsensus {
            rule: "unanimous".into(),
            readers: 5,
        },
        [SourceObservation::new("he_slide", "five reads")],
    )
    .expect("one source");

    assert_eq!(
        standard.admissible_tier(EvidenceTier::Deterministic),
        EvidenceTier::Statistical
    );
}

#[test]
fn a_standard_with_no_source_observation_is_refused() {
    let result = ReferenceStandard::new(
        "orphan",
        population(),
        ReferenceBasis::ClinicalRecord {
            source: "registry".into(),
        },
        [],
    );
    assert!(matches!(
        result,
        Err(OracleXError::StandardWithoutProcess { .. })
    ));
}

#[test]
fn a_borderline_call_is_a_distribution_and_has_no_single_class() {
    let mut mass = BTreeMap::new();
    mass.insert("entity_a".to_string(), 0.55);
    mass.insert("entity_b".to_string(), 0.45);
    let call = ClassCall::Spread { mass };

    assert_eq!(call.single_class(), None);
    assert_eq!(call.modes().len(), 1);
    assert!(ClassCall::Definite {
        class: "entity_a".into()
    }
    .single_class()
    .is_some());
}

#[test]
fn a_regrade_appends_and_the_earlier_revision_stays_retrievable() {
    let first = ReferenceStandard::new(
        "integrated-v1",
        population(),
        ReferenceBasis::IntegratedClassifier {
            classifier: "methylation".into(),
            classifier_version: "1".into(),
            ontology_version: "2021".into(),
        },
        [SourceObservation::new("array", "raw betas")],
    )
    .expect("one source")
    .with_call(ClassCall::Definite {
        class: "entity_a".into(),
    });
    let second = ReferenceStandard::new(
        "integrated-v1",
        population(),
        ReferenceBasis::IntegratedClassifier {
            classifier: "methylation".into(),
            classifier_version: "2".into(),
            ontology_version: "2024".into(),
        },
        [SourceObservation::new("array", "raw betas")],
    )
    .expect("one source")
    .with_call(ClassCall::Definite {
        class: "entity_b".into(),
    });

    let history = StandardHistory::begin(first.clone()).regrade(second, "classifier v2");

    assert_eq!(history.len(), 2);
    assert_eq!(history.at(0), Some(&first));
    assert!(history.calls_changed());
}

// ---- 31.12 endpoints -----------------------------------------------------------------------

#[test]
fn an_unranked_source_pair_leaves_the_date_unresolved_and_picks_neither() {
    let hierarchy = SourceHierarchy::new(["death_index"]);
    let claims = [
        DateClaim::new("death", at("2026-03-01T00:00:00Z"), "death_index"),
        DateClaim::new("death", at("2026-03-09T00:00:00Z"), "site_chart"),
    ];

    let reconciliation = reconcile(&claims, &hierarchy);

    assert!(!reconciliation.outcome.decided());
    assert_eq!(reconciliation.kept, None);
    assert_eq!(reconciliation.dropped.len(), 2);
}

#[test]
fn a_reconciliation_retains_the_claim_it_dropped() {
    let hierarchy = SourceHierarchy::new(["death_index", "site_chart"]);
    let claims = [
        DateClaim::new("death", at("2026-03-09T00:00:00Z"), "site_chart"),
        DateClaim::new("death", at("2026-03-01T00:00:00Z"), "death_index"),
    ];

    let reconciliation = reconcile(&claims, &hierarchy);

    assert!(reconciliation.outcome.is_supported());
    assert_eq!(
        reconciliation.kept.as_ref().map(|c| c.source.as_str()),
        Some("death_index")
    );
    assert_eq!(reconciliation.dropped.len(), 1);
    assert_eq!(override_witnesses(&reconciliation).len(), 1);
}

#[test]
fn an_event_after_the_last_contact_is_a_deterministic_contradiction() {
    let record = FollowUp::new(
        "S1",
        at("2026-01-01T00:00:00Z"),
        at("2026-02-01T00:00:00Z"),
        Outcome::Event {
            cause: "death".into(),
            at: at("2026-03-01T00:00:00Z"),
        },
    );
    assert_eq!(record.consistency().witnesses().len(), 1);
}

#[test]
fn an_unknown_outcome_has_no_date_and_therefore_no_censoring_time() {
    let record = FollowUp::new(
        "S1",
        at("2026-01-01T00:00:00Z"),
        at("2026-02-01T00:00:00Z"),
        Outcome::Unknown {
            reason: "site closed".into(),
        },
    );
    assert_eq!(record.outcome.at(), None);
    assert!(matches!(
        record.consistency(),
        bioprism_oraclex::Determination::NotEvaluable(_)
    ));
}

#[test]
fn an_unknown_eligibility_criterion_is_not_a_failed_one() {
    let unknown = Eligibility::new().met("age").unknown("biomarker").assess();
    assert!(!unknown.decided());
    assert!(unknown.witnesses().is_empty());

    let failed = Eligibility::new().met("age").failed("biomarker").assess();
    assert_eq!(failed.witnesses().len(), 1);

    let clear = Eligibility::new().met("age").met("biomarker").assess();
    assert!(clear.is_supported());
}

#[test]
fn assessments_under_different_criteria_versions_are_not_comparable() {
    let left = Assessment::new(ResponseCriteria::new("response", "1.1"), "partial");
    let right = Assessment::new(ResponseCriteria::new("response", "2.0"), "stable");
    assert!(!comparable(&left, &right).decided());

    let same = Assessment::new(ResponseCriteria::new("response", "1.1"), "stable");
    assert!(comparable(&left, &same).is_supported());
}

// ---- 31.17 audit ---------------------------------------------------------------------------

#[test]
fn no_party_may_both_author_the_benchmark_and_define_the_hidden_grading() {
    let assignment = RoleAssignment::new()
        .assign("lab_a", Role::BenchmarkAuthor)
        .assign("lab_a", Role::HiddenGrader);

    let determination = separation(&assignment);

    assert_eq!(determination.witnesses().len(), 1);
    assert_eq!(INCOMPATIBLE.len(), 4);
}

#[test]
fn a_sponsor_grading_alone_is_unresolved_until_someone_independent_reviews() {
    let alone = RoleAssignment::new().assign("lab_b", Role::HiddenGrader);
    assert!(!unilateral_control(&alone, Role::HiddenGrader).decided());

    let reviewed = alone.assign("lab_c", Role::IndependentReviewer);
    assert!(unilateral_control(&reviewed, Role::HiddenGrader).is_supported());
}

#[test]
fn a_withheld_negative_finding_contradicts_publication_integrity() {
    let clean = [
        ReportedFinding::positive("the method beat the baseline"),
        ReportedFinding::negative("the router captured no gain"),
    ];
    assert!(publication_integrity(&clean).is_supported());

    let suppressed = [ReportedFinding::negative("the router captured no gain")
        .withheld_by("sponsor")];
    assert_eq!(publication_integrity(&suppressed).witnesses().len(), 1);
}

#[test]
fn an_unattempted_reproduction_is_unresolved_and_not_a_failed_one() {
    let reference = ContentHash::of_bytes(b"reference output");
    let other = ContentHash::of_bytes(b"a different output");

    assert!(!independent_reproduction(&reference, None).decided());
    assert_eq!(
        independent_reproduction(&reference, Some(&other))
            .witnesses()
            .len(),
        1
    );
    assert!(independent_reproduction(&reference, Some(&reference)).is_supported());
}

