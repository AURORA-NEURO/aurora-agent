//! Blueprint 15.19 — a pack can be unhealthy, and an unhealthy pack yields no number.
//!
//! The claim each test makes is in its name. Together they establish the property the crate
//! exists for: there is no path from a saturated, degenerate or contaminated pack to a published
//! score.

use bioprism_packs::{
    assess, AgentCapability, CalibrationPolicy, CapabilityFamily, ContaminationSignal,
    DifficultyCalibration, Discrimination, Domain, HealthFinding, HealthPolicy, HealthVerdict,
    InstanceSource, Observations, OracleTier, PackAxis, PackContent, PackError, PackId, PackIr,
    PackManifest, PackVersion, ParentEnvironment, SchemaRange, SeedRange, Severity,
    SystemObservation, TrivialBaseline, WorldId,
};

fn pack_with(oracles: Vec<OracleTier>, instances: InstanceSource) -> PackIr {
    PackIr {
        manifest: PackManifest {
            id: PackId::parse("prism.verification-recovery").unwrap(),
            version: PackVersion::new(1, 0, 0),
            schema_range: SchemaRange::new(1, 1),
            title: "Verification, Recovery and Backtracking".into(),
            measures: "Whether agents detect silent failure and recover without compounding harm."
                .into(),
            blueprint_module: "15.05".into(),
            axis: PackAxis::Mechanism,
            capabilities: vec![CapabilityFamily::Agent(
                AgentCapability::VerificationAndRecovery,
            )],
            domains: vec![Domain::Coding],
            owners: vec!["prism-core".into()],
            license: "Apache-2.0".into(),
            dependencies: Vec::new(),
        },
        content: PackContent {
            parent_environments: vec![ParentEnvironment {
                world: WorldId::parse("world-fault-001").unwrap(),
                decision_parents: 30,
            }],
            decision_families: vec!["select the next verifier".into()],
            mutation_relations: vec!["fault-severity-ladder".into()],
            oracles,
            instances,
            executed_trials: 900,
            independent_reproductions: 1,
            effective_sample_size: Some(40),
        },
    }
}

fn healthy_pack() -> PackIr {
    pack_with(
        vec![OracleTier::Deterministic, OracleTier::Executable],
        InstanceSource::Authored { validated: 900 },
    )
}

fn calibration(rates: &[(&str, u32, u32)]) -> DifficultyCalibration {
    DifficultyCalibration::new(
        rates
            .iter()
            .map(|(system, passes, trials)| {
                SystemObservation::new(*system, *passes, *trials).unwrap()
            })
            .collect(),
    )
}

fn discriminating() -> DifficultyCalibration {
    calibration(&[("a", 30, 100), ("b", 55, 100), ("c", 80, 100)])
}

#[test]
fn a_pack_where_everything_passes_is_flagged_saturated_and_not_reported_as_a_score() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: calibration(&[("a", 99, 100), ("b", 98, 100), ("c", 100, 100)]),
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert!(matches!(
        assessment.health.findings.first(),
        Some(HealthFinding::Saturated { .. })
    ));
    assert_eq!(assessment.health.verdict(), HealthVerdict::Unreportable);

    let error = assessment.reportable_score(&pack).unwrap_err();
    match error {
        PackError::UnreportablePack { findings, .. } => assert!(findings.contains("saturated")),
        other => panic!("a saturated pack must not yield a score, got {other:?}"),
    }
}

#[test]
fn a_pack_where_everything_fails_is_flagged_floored_and_discriminates_nothing() {
    let pack = healthy_pack();
    let floored = calibration(&[("a", 1, 100), ("b", 0, 100), ("c", 3, 100)]);

    assert!(matches!(
        floored.discrimination(&CalibrationPolicy::default()),
        Discrimination::Floored { .. }
    ));

    let assessment = assess(
        &pack,
        &Observations {
            calibration: floored,
            ..Observations::default()
        },
        &HealthPolicy::default(),
    )
    .unwrap();

    assert!(matches!(
        assessment.health.findings.first(),
        Some(HealthFinding::Floored { .. })
    ));
    assert!(assessment.reportable_score(&pack).is_err());
}

#[test]
fn a_trivial_heuristic_matching_the_best_system_flags_the_pack_degenerate() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        trivial_baselines: vec![TrivialBaseline {
            name: "always answer the first listed option".into(),
            observation: SystemObservation::new("first-option", 78, 100).unwrap(),
        }],
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    let degenerate = assessment
        .health
        .findings
        .iter()
        .find(|f| matches!(f, HealthFinding::Degenerate { .. }))
        .expect("a heuristic within the margin of the best system is degeneracy");
    assert_eq!(degenerate.severity(), Severity::Blocking);
    assert!(assessment.reportable_score(&pack).is_err());
}

#[test]
fn a_trivial_heuristic_far_below_the_systems_leaves_the_pack_reportable() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        trivial_baselines: vec![TrivialBaseline {
            name: "always abstain".into(),
            observation: SystemObservation::new("abstain", 8, 100).unwrap(),
        }],
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert!(assessment.health.findings.is_empty());
    assert_eq!(assessment.health.verdict(), HealthVerdict::Healthy);
    assert!(assessment.reportable_score(&pack).is_ok());
}

#[test]
fn a_memorization_gap_above_the_margin_blocks_publication() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        contamination: vec![ContaminationSignal::MemorizationGap {
            public: SystemObservation::new("model-x public", 91, 100).unwrap(),
            held_out: SystemObservation::new("model-x held out", 44, 100).unwrap(),
        }],
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert!(assessment
        .health
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::Contaminated { .. })));
    assert!(assessment.reportable_score(&pack).is_err());
}

#[test]
fn a_memorization_gap_below_the_margin_is_not_reported_as_contamination() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        contamination: vec![ContaminationSignal::MemorizationGap {
            public: SystemObservation::new("model-x public", 57, 100).unwrap(),
            held_out: SystemObservation::new("model-x held out", 55, 100).unwrap(),
        }],
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert!(assessment.health.findings.is_empty());
    assert!(assessment.reportable_score(&pack).is_ok());
}

#[test]
fn a_pack_released_before_a_model_cutoff_is_contaminated_without_needing_a_pass_rate_gap() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        contamination: vec![ContaminationSignal::ReleasedBeforeCutoff {
            pack_release: "2024-02-01".into(),
            model_cutoff: "2025-06-01".into(),
        }],
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();
    assert_eq!(assessment.health.verdict(), HealthVerdict::Unreportable);
}

#[test]
fn a_pack_whose_only_oracles_are_nondeterministic_cannot_publish_a_score() {
    let pack = pack_with(
        vec![OracleTier::ExpertReview, OracleTier::Rubric],
        InstanceSource::Authored { validated: 400 },
    );
    let observations = Observations {
        calibration: discriminating(),
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert!(assessment
        .health
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::NoGroundedOracle { .. })));
    assert!(assessment.reportable_score(&pack).is_err());
}

#[test]
fn an_assessment_taken_against_a_different_pack_revision_cannot_publish_a_score() {
    let original = healthy_pack();
    let observations = Observations {
        calibration: discriminating(),
        ..Observations::default()
    };
    let assessment = assess(&original, &observations, &HealthPolicy::default()).unwrap();

    let mut revised = healthy_pack();
    revised.content.parent_environments.push(ParentEnvironment {
        world: WorldId::parse("world-fault-002").unwrap(),
        decision_parents: 12,
    });

    assert!(assessment.reportable_score(&original).is_ok());
    assert!(matches!(
        assessment.reportable_score(&revised).unwrap_err(),
        PackError::AssessmentDigestMismatch { .. }
    ));
}

#[test]
fn a_declared_instance_count_that_was_never_validated_is_advisory_and_travels_with_the_score() {
    let pack = pack_with(
        vec![OracleTier::Executable],
        InstanceSource::DeterministicGenerator {
            seeds: SeedRange::new(0, 4_000_000),
            declared: 1_000_000,
            validated: 2_000,
        },
    );
    let observations = Observations {
        calibration: discriminating(),
        ..Observations::default()
    };

    let assessment = assess(&pack, &observations, &HealthPolicy::default()).unwrap();

    assert_eq!(assessment.health.verdict(), HealthVerdict::Degraded);
    let score = assessment.reportable_score(&pack).unwrap();
    assert!(score
        .advisories
        .iter()
        .any(|f| matches!(f, HealthFinding::CountsNotMaterialized { .. })));
}

#[test]
fn a_pack_with_no_recorded_trials_has_no_pass_rate_rather_than_a_pass_rate_of_zero() {
    let pack = healthy_pack();
    let assessment = assess(&pack, &Observations::default(), &HealthPolicy::default()).unwrap();

    assert!(matches!(
        assessment.health.findings.first(),
        Some(HealthFinding::NotYetCharacterised { .. })
    ));
    assert_eq!(assessment.health.verdict(), HealthVerdict::Degraded);
    assert!(matches!(
        assessment.reportable_score(&pack).unwrap_err(),
        PackError::NoObservations(_)
    ));
}

#[test]
fn passes_exceeding_trials_is_a_typed_error_not_a_pass_rate_above_one() {
    assert!(matches!(
        SystemObservation::new("impossible", 11, 10).unwrap_err(),
        PackError::ImpossibleObservation { .. }
    ));
}

#[test]
fn two_systems_whose_wilson_intervals_overlap_do_not_establish_an_ordering() {
    let close = calibration(&[("a", 10, 20), ("b", 11, 20), ("c", 12, 20)]);
    match close.discrimination(&CalibrationPolicy::default()) {
        Discrimination::Discriminating { separated, .. } => assert!(
            !separated,
            "20 trials apiece cannot separate 0.50 from 0.60"
        ),
        other => panic!("expected a discriminating verdict, got {other:?}"),
    }

    match discriminating().discrimination(&CalibrationPolicy::default()) {
        Discrimination::Discriminating {
            lowest,
            highest,
            separated,
        } => {
            assert!(separated, "0.30 and 0.80 over 100 trials are separated");
            assert!((lowest - 0.30).abs() < 1e-12);
            assert!((highest - 0.80).abs() < 1e-12);
        }
        other => panic!("expected a discriminating verdict, got {other:?}"),
    }
}

#[test]
fn one_system_yields_an_undetermined_discrimination_rather_than_a_range() {
    let single = calibration(&[("only-system", 50, 100)]);
    match single.discrimination(&CalibrationPolicy::default()) {
        Discrimination::Undetermined { reason } => assert!(reason.contains("system")),
        other => panic!("one system cannot characterise a pack, got {other:?}"),
    }
    assert!(single
        .report(&CalibrationPolicy::default())
        .contains("undetermined"));
}

#[test]
fn systems_with_too_few_trials_do_not_count_toward_characterising_the_pack() {
    let thin = calibration(&[("a", 3, 5), ("b", 2, 5), ("c", 4, 5)]);
    assert!(matches!(
        thin.discrimination(&CalibrationPolicy::default()),
        Discrimination::Undetermined { .. }
    ));
}

#[test]
fn a_nondeterministic_oracle_may_not_override_an_execution_grounded_one() {
    assert!(!OracleTier::Rubric.may_override(OracleTier::Executable));
    assert!(!OracleTier::ExpertReview.may_override(OracleTier::Deterministic));
    assert!(!OracleTier::Rubric.may_override(OracleTier::PolicyVeto));
    assert!(OracleTier::Deterministic.may_override(OracleTier::Rubric));
    assert!(OracleTier::Rubric.may_override(OracleTier::ExpertReview));
    assert!(OracleTier::Statistical.may_override(OracleTier::Statistical));
}

#[test]
fn health_findings_round_trip_through_json_so_a_report_can_be_archived() {
    let findings = vec![
        HealthFinding::Saturated {
            pooled_pass_rate: 0.985,
            systems: 4,
        },
        HealthFinding::Degenerate {
            baseline: "echo the input".into(),
            baseline_pass_rate: 0.62,
            best_system_pass_rate: 0.64,
        },
        HealthFinding::Contaminated {
            signal: ContaminationSignal::CorpusMembership {
                corpus: "public-web-2025".into(),
                matched_instances: 812,
            },
        },
        HealthFinding::NoGroundedOracle {
            tiers: vec![OracleTier::Rubric],
        },
    ];

    let encoded = serde_json::to_string(&findings).unwrap();
    let decoded: Vec<HealthFinding> = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, findings);
    for finding in &decoded {
        assert!(!finding.explain().is_empty());
    }
}

#[test]
fn the_saturation_ceiling_is_a_policy_choice_and_moving_it_moves_the_verdict() {
    let pack = healthy_pack();
    let observations = Observations {
        calibration: calibration(&[("a", 90, 100), ("b", 91, 100), ("c", 92, 100)]),
        ..Observations::default()
    };

    let default_policy = HealthPolicy::default();
    assert!(assess(&pack, &observations, &default_policy)
        .unwrap()
        .health
        .findings
        .iter()
        .all(|f| !matches!(f, HealthFinding::Saturated { .. })));

    let strict = HealthPolicy {
        calibration: CalibrationPolicy {
            saturation_ceiling: 0.85,
            ..CalibrationPolicy::default()
        },
        ..HealthPolicy::default()
    };
    assert!(assess(&pack, &observations, &strict)
        .unwrap()
        .health
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::Saturated { .. })));
}
