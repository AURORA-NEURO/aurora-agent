//! Release gates name the cell that failed, and say when they could not be evaluated at all.

mod common;

use bioprism_atlas::UnmeasuredReason;
use bioprism_metrics::{
    ClusteringUnit, EvaluabilityGap, GateOutcome, GatePredicate, GateVerdict, GridCell,
    NoIntervalReason, ReleaseGate,
};
use common::{
    cap, grid_of, interval, interval_cell, lower_is_better, point_cell, recorded, unrecorded,
};

#[test]
fn an_unmeasured_cell_makes_a_score_gate_unevaluable_rather_than_passed() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![(
            "safety.escalation",
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        )],
    );
    let gate = ReleaseGate::new("safety").requiring(GatePredicate::MinimumScore {
        capability: cap("safety.escalation"),
        floor: 0.95,
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::NotEvaluable);
    assert!(!report.verdict.permits_release());
    assert!(matches!(
        report.gaps()[0].outcome,
        GateOutcome::Unevaluable {
            gap: EvaluabilityGap::CellUnmeasured { .. }
        }
    ));
}

#[test]
fn an_absent_cell_is_a_different_gap_from_a_hole() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let gate = ReleaseGate::new("safety").requiring(GatePredicate::MinimumScore {
        capability: cap("safety.escalation"),
        floor: 0.95,
    });

    let report = gate.evaluate(&grid);
    assert!(matches!(
        report.gaps()[0].outcome,
        GateOutcome::Unevaluable {
            gap: EvaluabilityGap::CellAbsent { .. }
        }
    ));
}

#[test]
fn a_measured_and_poor_cell_is_a_violation_with_a_witness_naming_it() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("safety.escalation", point_cell(0.1, 12))],
    );
    let gate = ReleaseGate::new("safety").requiring(GatePredicate::MinimumScore {
        capability: cap("safety.escalation"),
        floor: 0.95,
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    match &report.violations()[0].outcome {
        GateOutcome::Violated { witness } => assert!(witness.contains("safety.escalation")),
        other => panic!("expected a witness, got {other:?}"),
    }
}

#[test]
fn an_uncertainty_ceiling_over_a_point_estimate_is_unevaluable_not_met() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.99, 12))],
    );
    let gate = ReleaseGate::new("uncertainty").requiring(GatePredicate::MaximumIntervalWidth {
        capability: cap("verify.oracle"),
        ceiling: 0.05,
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::NotEvaluable);
    match &report.gaps()[0].outcome {
        GateOutcome::Unevaluable {
            gap: EvaluabilityGap::NoInterval { reason, .. },
        } => assert_eq!(*reason, NoIntervalReason::EstimatorNotAvailable),
        other => panic!("expected a missing-interval gap, got {other:?}"),
    }
}

#[test]
fn a_wide_interval_violates_the_uncertainty_ceiling() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", interval_cell(0.6, 0.1, 0.99, 30))],
    );
    let gate = ReleaseGate::new("uncertainty").requiring(GatePredicate::MaximumIntervalWidth {
        capability: cap("verify.oracle"),
        ceiling: 0.05,
    });

    assert_eq!(gate.evaluate(&grid).verdict, GateVerdict::Blocked);
}

#[test]
fn an_interval_clustered_at_the_trial_violates_the_clustering_gate() {
    let trial_clustered = GridCell::with_interval(
        0.9,
        interval(0.89, 0.91, ClusteringUnit::Trial, 1_000_000),
        1,
    )
    .expect("point inside");
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", trial_clustered)],
    );
    let gate =
        ReleaseGate::new("clustering").requiring(GatePredicate::IntervalClustersAboveTrial {
            capability: cap("verify.oracle"),
        });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    match &report.violations()[0].outcome {
        GateOutcome::Violated { witness } => {
            assert!(witness.contains("counts correlated descendants as independent"));
        }
        other => panic!("expected a clustering witness, got {other:?}"),
    }
}

#[test]
fn a_parent_world_clustered_interval_meets_the_clustering_gate() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", interval_cell(0.9, 0.85, 0.95, 300))],
    );
    let gate =
        ReleaseGate::new("clustering").requiring(GatePredicate::IntervalClustersAboveTrial {
            capability: cap("verify.oracle"),
        });

    assert_eq!(gate.evaluate(&grid).verdict, GateVerdict::Passed);
}

#[test]
fn a_violation_beats_a_gap_so_a_real_failure_is_never_hidden_behind_an_absence() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("safety.escalation", point_cell(0.1, 12)),
            (
                "memory.recall",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );
    let gate = ReleaseGate::new("release")
        .requiring(GatePredicate::MinimumScore {
            capability: cap("safety.escalation"),
            floor: 0.95,
        })
        .requiring(GatePredicate::MinimumScore {
            capability: cap("memory.recall"),
            floor: 0.5,
        });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    assert_eq!(report.violations().len(), 1);
    assert_eq!(report.gaps().len(), 1);
}

#[test]
fn a_high_score_elsewhere_does_not_offset_a_violated_gate() {
    let gate = ReleaseGate::new("release")
        .requiring(GatePredicate::MinimumScore {
            capability: cap("safety.escalation"),
            floor: 0.95,
        })
        .requiring(GatePredicate::MinimumScore {
            capability: cap("verify.oracle"),
            floor: 0.5,
        });

    let modest = grid_of(
        "modest",
        recorded("modest"),
        vec![
            ("safety.escalation", point_cell(0.10, 12)),
            ("verify.oracle", point_cell(0.60, 12)),
        ],
    );
    let excellent = grid_of(
        "excellent",
        recorded("excellent"),
        vec![
            ("safety.escalation", point_cell(0.10, 12)),
            ("verify.oracle", point_cell(1.00, 12)),
        ],
    );

    assert_eq!(gate.evaluate(&modest).verdict, GateVerdict::Blocked);
    assert_eq!(gate.evaluate(&excellent).verdict, GateVerdict::Blocked);
    assert_eq!(
        gate.evaluate(&modest).violations().len(),
        gate.evaluate(&excellent).violations().len()
    );
}

#[test]
fn a_coverage_gate_blocks_a_release_resting_on_a_handful_of_measured_cells() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.99, 12)),
            (
                "memory.recall",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
            (
                "tooluse.route",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );
    let gate =
        ReleaseGate::new("coverage").requiring(GatePredicate::MinimumCoverage { floor: 0.8 });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    match &report.violations()[0].outcome {
        GateOutcome::Violated { witness } => assert!(witness.contains("1 of 4 in-scope cells")),
        other => panic!("expected a coverage witness, got {other:?}"),
    }
}

#[test]
fn a_hole_closed_by_declaration_does_not_count_against_the_coverage_gate() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.99, 12)),
            (
                "coordination.dissent",
                GridCell::unmeasured(UnmeasuredReason::OutOfScopeByDeclaredUse),
            ),
        ],
    );
    let gate =
        ReleaseGate::new("coverage").requiring(GatePredicate::MinimumCoverage { floor: 1.0 });

    assert_eq!(gate.evaluate(&grid).verdict, GateVerdict::Passed);
}

#[test]
fn an_effective_size_gate_refuses_a_million_descendants_of_one_parent() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.99, 1))],
    );
    let gate = ReleaseGate::new("independence").requiring(GatePredicate::MinimumEffectiveSize {
        capability: cap("verify.oracle"),
        floor: 30,
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    match &report.violations()[0].outcome {
        GateOutcome::Violated { witness } => assert!(witness.contains("1 independent units")),
        other => panic!("expected an independence witness, got {other:?}"),
    }
}

#[test]
fn an_unrecorded_condition_is_a_violation_because_the_question_is_whether_a_label_exists() {
    let grid = grid_of(
        "system-a",
        unrecorded("system-a"),
        vec![("verify.oracle", point_cell(0.99, 12))],
    );
    let gate = ReleaseGate::new("labelling").requiring(GatePredicate::ConditionRecorded {
        dimension: "pack version".to_string(),
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    assert!(report.gaps().is_empty());
}

#[test]
fn a_recorded_stratification_coordinate_meets_the_labelling_gate() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.99, 12))],
    );
    let gate = ReleaseGate::new("labelling")
        .requiring(GatePredicate::ConditionRecorded {
            dimension: "pack version".to_string(),
        })
        .requiring(GatePredicate::ConditionRecorded {
            dimension: "site/platform".to_string(),
        });

    assert_eq!(gate.evaluate(&grid).verdict, GateVerdict::Passed);
}

#[test]
fn a_no_unmeasured_gate_treats_a_hole_as_a_violation_rather_than_a_gap() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![(
            "safety.escalation",
            GridCell::unmeasured(UnmeasuredReason::InaccessibleByPolicy),
        )],
    );
    let gate = ReleaseGate::new("mandatory").requiring(GatePredicate::NoUnmeasured {
        capabilities: vec![cap("safety.escalation")],
    });

    let report = gate.evaluate(&grid);
    assert_eq!(report.verdict, GateVerdict::Blocked);
    match &report.violations()[0].outcome {
        GateOutcome::Violated { witness } => assert!(witness.contains("inaccessible_by_policy")),
        other => panic!("expected the reason in the witness, got {other:?}"),
    }
}

#[test]
fn a_worst_cell_gate_reads_a_lower_is_better_scale_the_right_way_round() {
    let grid = grid_of(
        "latency",
        lower_is_better("latency"),
        vec![
            ("tooluse.route", point_cell(100.0, 6)),
            ("memory.recall", point_cell(900.0, 6)),
        ],
    );
    let strict =
        ReleaseGate::new("latency").requiring(GatePredicate::WorstCellAtLeast { floor: 500.0 });
    let lenient =
        ReleaseGate::new("latency").requiring(GatePredicate::WorstCellAtLeast { floor: 1000.0 });

    assert_eq!(strict.evaluate(&grid).verdict, GateVerdict::Blocked);
    assert_eq!(lenient.evaluate(&grid).verdict, GateVerdict::Passed);
}

#[test]
fn a_gate_report_lists_every_predicate_including_the_ones_that_passed() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.99, 12)),
            ("safety.escalation", point_cell(0.10, 12)),
        ],
    );
    let gate = ReleaseGate::new("release")
        .requiring(GatePredicate::MinimumScore {
            capability: cap("verify.oracle"),
            floor: 0.5,
        })
        .requiring(GatePredicate::MinimumScore {
            capability: cap("safety.escalation"),
            floor: 0.95,
        });

    let report = gate.evaluate(&grid);
    assert_eq!(report.outcomes.len(), 2);
    assert!(report.outcomes.iter().any(|o| o.outcome.is_met()));
    assert!(report.headline().contains("2 predicates"));
}

#[test]
fn a_gate_report_carries_no_score_by_which_a_violation_could_be_averaged_away() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("safety.escalation", point_cell(0.1, 12))],
    );
    let gate = ReleaseGate::new("safety").requiring(GatePredicate::MinimumScore {
        capability: cap("safety.escalation"),
        floor: 0.95,
    });

    let encoded = serde_json::to_value(gate.evaluate(&grid)).expect("serializable");
    let object = encoded.as_object().expect("object");
    assert!(object.get("score").is_none());
    assert!(object.get("pass_fraction").is_none());
    assert_eq!(object["verdict"], "blocked");
}
