//! Aggregation carries its own coverage, or it does not exist.

mod common;

use bioprism_atlas::UnmeasuredReason;
use bioprism_metrics::Subject;
use bioprism_metrics::{
    AggregationRule, CoveredAggregate, CoveredAggregateFields, DeclaredWeighting, Estimate,
    GridCell, MetricsError,
};
use common::{cap, grid_of, interval_cell, lower_is_better, point_cell, recorded};

#[test]
fn an_aggregate_over_a_grid_with_an_unmeasured_cell_cannot_be_reported_as_a_bare_score() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );

    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");
    assert!(!aggregate.covers_its_grid());

    let encoded = serde_json::to_value(&aggregate).expect("serializable");
    let coverage = &encoded["coverage"];
    assert_eq!(
        coverage["blocking_holes"][0]["capability"],
        "safety.escalation"
    );
    assert_eq!(coverage["contributed"][0], "verify.oracle");
    assert_eq!(coverage["cells"], 2);
    assert!(encoded.get("estimate").is_some());
}

#[test]
fn a_grid_with_no_measured_cell_yields_no_aggregate_rather_than_zero() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            (
                "verify.oracle",
                GridCell::unmeasured(UnmeasuredReason::AllTrialsNonEvaluable),
            ),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );

    assert!(matches!(
        CoveredAggregate::mean(&grid),
        Err(MetricsError::AggregateOverNothing { .. })
    ));
}

#[test]
fn a_complete_mean_refuses_and_names_the_cells_that_are_unmeasured() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::DeferredAcquisition),
            ),
        ],
    );

    match CoveredAggregate::complete_mean(&grid) {
        Err(MetricsError::IncompleteGrid { unmeasured, .. }) => {
            assert_eq!(unmeasured, vec!["safety.escalation".to_string()]);
        }
        other => panic!("expected a refusal naming the hole, got {other:?}"),
    }
}

#[test]
fn a_hole_the_declared_use_closes_does_not_block_a_complete_mean() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "coordination.dissent",
                GridCell::unmeasured(UnmeasuredReason::OutOfScopeByDeclaredUse),
            ),
        ],
    );

    let aggregate =
        CoveredAggregate::complete_mean(&grid).expect("out-of-scope holes do not block");
    assert!(aggregate.coverage().is_complete());
    assert_eq!(aggregate.coverage().holes_closed_by_declaration.len(), 1);
    assert_eq!(aggregate.coverage().fraction_of_scope, 1.0);
    assert_eq!(aggregate.coverage().fraction_of_grid, 0.5);
}

#[test]
fn coverage_fractions_are_rederived_on_deserialization_so_a_document_cannot_overstate_them() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields.coverage.fraction_of_grid = 1.0;
    fields.coverage.fraction_of_scope = 1.0;
    let document = serde_json::to_string(&fields).expect("serializable");

    let restored: CoveredAggregate = serde_json::from_str(&document).expect("valid accounting");
    assert_eq!(restored.coverage().fraction_of_grid, 0.5);
    assert_eq!(restored.coverage().fraction_of_scope, 0.5);
}

#[test]
fn a_deserialized_aggregate_whose_cells_do_not_add_up_is_refused() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields.coverage.cells = 40;
    let document = serde_json::to_string(&fields).expect("serializable");

    let error = serde_json::from_str::<CoveredAggregate>(&document)
        .expect_err("40 cells cannot be accounted for by one contribution");
    assert!(error.to_string().contains("40"));
}

#[test]
fn a_deserialized_aggregate_with_no_contributing_cell_is_refused() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields.coverage.contributed.clear();
    fields.coverage.cells = 0;
    let document = serde_json::to_string(&fields).expect("serializable");

    assert!(serde_json::from_str::<CoveredAggregate>(&document).is_err());
}

#[test]
fn a_deserialized_aggregate_cannot_place_one_capability_in_two_coverage_buckets() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields
        .coverage
        .measured_but_excluded
        .push(cap("verify.oracle"));
    let document = serde_json::to_string(&fields).expect("serializable");

    assert!(matches!(
        serde_json::from_str::<CoveredAggregate>(&document),
        Err(error) if error.to_string().contains("more than once")
    ));
}

#[test]
fn a_deserialized_aggregate_must_keep_its_grid_subject() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields.conditions.subject = Subject::grid("system-b");
    let document = serde_json::to_string(&fields).expect("serializable");

    assert!(matches!(
        serde_json::from_str::<CoveredAggregate>(&document),
        Err(error) if error.to_string().contains("has subject")
    ));
}

#[test]
fn a_deserialized_worst_aggregate_must_bind_its_worst_value_to_its_estimate() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            ("memory.recall", point_cell(0.2, 12)),
        ],
    );
    let aggregate = CoveredAggregate::worst(&grid).expect("two cells contributed");

    let mut fields: CoveredAggregateFields = aggregate.into();
    fields.worst.value = 0.3;
    let document = serde_json::to_string(&fields).expect("serializable");

    assert!(matches!(
        serde_json::from_str::<CoveredAggregate>(&document),
        Err(error) if error.to_string().contains("does not match worst-cell value")
    ));
}

#[test]
fn a_weighted_aggregate_refuses_when_a_weighted_capability_is_unmeasured() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );
    let weighting = DeclaredWeighting::declare(
        "triage assistant",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 2.0)],
    )
    .expect("valid weighting");

    match CoveredAggregate::weighted(&grid, &weighting) {
        Err(MetricsError::WeightedCapabilityUnmeasured { capability, .. }) => {
            assert_eq!(capability, "safety.escalation");
        }
        other => panic!("expected a refusal naming the hole, got {other:?}"),
    }
}

#[test]
fn a_weighted_aggregate_refuses_when_a_weighted_capability_is_absent_from_the_grid() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![("verify.oracle", point_cell(0.9, 12))],
    );
    let weighting =
        DeclaredWeighting::declare("triage assistant", vec![(cap("memory.recall"), 1.0)])
            .expect("valid weighting");

    assert!(matches!(
        CoveredAggregate::weighted(&grid, &weighting),
        Err(MetricsError::WeightedCapabilityAbsent { .. })
    ));
}

#[test]
fn an_aggregate_records_the_measured_cells_its_rule_excluded() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            ("memory.recall", point_cell(0.4, 12)),
        ],
    );
    let weighting =
        DeclaredWeighting::declare("verification only", vec![(cap("verify.oracle"), 1.0)])
            .expect("valid weighting");

    let aggregate = CoveredAggregate::weighted(&grid, &weighting).expect("weighted cell measured");
    assert_eq!(
        aggregate.coverage().measured_but_excluded,
        vec![cap("memory.recall")]
    );
    assert_eq!(aggregate.coverage().fraction_of_grid, 0.5);
}

#[test]
fn the_worst_cell_aggregate_names_the_loser_while_counting_every_measured_cell() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            ("memory.recall", point_cell(0.2, 5)),
            ("tooluse.route", point_cell(0.7, 8)),
        ],
    );

    let aggregate = CoveredAggregate::worst(&grid).expect("three measured cells");
    assert_eq!(aggregate.worst_cell().capability, cap("memory.recall"));
    assert_eq!(aggregate.value().get(), 0.2);
    assert_eq!(aggregate.coverage().contributed.len(), 3);
    assert_eq!(*aggregate.rule(), AggregationRule::WorstMeasured);
}

#[test]
fn under_a_lower_is_better_rule_the_worst_cell_is_the_largest_value() {
    let grid = grid_of(
        "latency",
        lower_is_better("latency"),
        vec![
            ("tooluse.route", point_cell(120.0, 6)),
            ("memory.recall", point_cell(900.0, 6)),
        ],
    );

    let aggregate = CoveredAggregate::worst(&grid).expect("two measured cells");
    assert_eq!(aggregate.worst_cell().capability, cap("memory.recall"));
    assert_eq!(aggregate.value().get(), 900.0);
}

#[test]
fn an_aggregate_rests_on_the_smallest_effective_size_not_the_sum() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 300)),
            ("memory.recall", point_cell(0.8, 2)),
        ],
    );

    let aggregate = CoveredAggregate::mean(&grid).expect("two measured cells");
    assert_eq!(aggregate.effective_size(), 2);
}

#[test]
fn an_aggregate_of_interval_cells_carries_an_interval_of_its_own() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", interval_cell(0.9, 0.85, 0.95, 30)),
            ("memory.recall", interval_cell(0.7, 0.60, 0.80, 30)),
        ],
    );

    let aggregate = CoveredAggregate::mean(&grid).expect("two measured cells");
    let interval = aggregate.interval().expect("both cells had intervals");
    assert!((interval.low() - 0.725).abs() < 1e-9);
    assert!((interval.high() - 0.875).abs() < 1e-9);
    assert!(interval.width() > 0.0);
}

#[test]
fn one_cell_without_an_interval_costs_the_aggregate_its_interval_and_says_so() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", interval_cell(0.9, 0.85, 0.95, 30)),
            ("memory.recall", point_cell(0.7, 30)),
        ],
    );

    let aggregate = CoveredAggregate::mean(&grid).expect("two measured cells");
    assert!(aggregate.interval().is_none());
    assert!(matches!(aggregate.estimate(), Estimate::Point { .. }));
    assert!(aggregate.estimate().no_interval_reason().is_some());
}

#[test]
fn a_weighted_aggregate_records_the_digest_of_the_weighting_that_produced_it() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            ("memory.recall", point_cell(0.5, 12)),
        ],
    );
    let weighting = DeclaredWeighting::declare(
        "triage assistant",
        vec![(cap("verify.oracle"), 3.0), (cap("memory.recall"), 1.0)],
    )
    .expect("valid weighting");

    let aggregate = CoveredAggregate::weighted(&grid, &weighting).expect("both cells measured");
    assert_eq!(
        aggregate.weighting_digest(),
        Some(weighting.digest().as_str())
    );
    assert!((aggregate.value().get() - 0.8).abs() < 1e-9);
}

#[test]
fn the_spread_of_an_aggregate_distinguishes_a_flat_grid_from_a_lopsided_one() {
    let flat = grid_of(
        "flat",
        recorded("flat"),
        vec![
            ("verify.oracle", point_cell(0.70, 9)),
            ("memory.recall", point_cell(0.70, 9)),
        ],
    );
    let lopsided = grid_of(
        "lopsided",
        recorded("lopsided"),
        vec![
            ("verify.oracle", point_cell(0.99, 9)),
            ("memory.recall", point_cell(0.41, 9)),
        ],
    );

    let flat = CoveredAggregate::mean(&flat).expect("measured");
    let lopsided = CoveredAggregate::mean(&lopsided).expect("measured");
    assert!((flat.value().get() - lopsided.value().get()).abs() < 1e-9);
    assert_eq!(flat.spread(), 0.0);
    assert!(lopsided.spread() > 0.5);
}

#[test]
fn the_coverage_headline_states_the_rule_it_is_enforcing() {
    let grid = grid_of(
        "system-a",
        recorded("system-a"),
        vec![
            ("verify.oracle", point_cell(0.9, 12)),
            (
                "safety.escalation",
                GridCell::unmeasured(UnmeasuredReason::NotAttempted),
            ),
        ],
    );
    let aggregate = CoveredAggregate::mean(&grid).expect("one cell contributed");
    let headline = aggregate.coverage().headline();
    assert!(headline.contains("1 of 2 cells contributed"));
    assert!(headline.contains("is not an aggregate over the grid"));
}
