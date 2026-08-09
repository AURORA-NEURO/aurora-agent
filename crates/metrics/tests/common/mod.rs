#![allow(dead_code)]

//! Fixtures shared by the metric suites.
//!
//! Every grid here is hand-built. Nothing in this crate measures anything, so a fixture that looked
//! like a real evaluation result would be the one dishonest object in the repository.

use bioprism_atlas::{CapabilityId, OracleTier};
use bioprism_metrics::{
    Budget, CapabilityGrid, ClusteringUnit, ConfidenceLevel, Direction, GridCell, Interval,
    IntervalBasis, MeasurementConditions, NoIntervalReason, ScoringRule, Stratum, Subject,
};

pub fn cap(id: &str) -> CapabilityId {
    CapabilityId::parse(id).expect("well-formed capability id")
}

/// Conditions with every coordinate this crate models recorded.
pub fn recorded(label: &str) -> MeasurementConditions {
    MeasurementConditions::new(Subject::grid(label), ScoringRule::atlas_pass_rate())
        .with_ontology_version("test-ontology/1")
        .with_pack_version("pack/4")
        .with_evidence_base("public-observed/2026-01")
        .with_oracle_floor(OracleTier::Executable)
        .with_budget(Budget::labelled("standard").with_tokens(100_000))
        .with_stratum(
            Stratum::new()
                .with("system version", "1.0.0")
                .with("architecture version", "a1")
                .with("model version", "m1")
                .with("parent world", "w1")
                .with("decision family", "prognosis")
                .with("biological scale", "tissue")
                .with("modality", "imaging")
                .with("disease entity", "glioma")
                .with("site/platform", "site-a")
                .with("population/time stratum", "2026-h1")
                .with("mutation family", "paraphrase"),
        )
}

/// Conditions in which nothing was written down. The state most real evaluations start in.
pub fn unrecorded(label: &str) -> MeasurementConditions {
    MeasurementConditions::new(Subject::grid(label), ScoringRule::atlas_pass_rate())
}

pub fn lower_is_better(label: &str) -> MeasurementConditions {
    let mut conditions = recorded(label);
    conditions.scoring_rule = ScoringRule::new(
        "wall-clock latency",
        Direction::LowerIsBetter,
        "milliseconds",
    );
    conditions
}

pub fn interval(low: f64, high: f64, unit: ClusteringUnit, effective: usize) -> Interval {
    Interval::new(
        low,
        high,
        ConfidenceLevel::ninety_five(),
        IntervalBasis::new("caller-supplied", unit, effective),
    )
    .expect("well-formed interval")
}

pub fn point_cell(value: f64, effective: usize) -> GridCell {
    GridCell::point(value, NoIntervalReason::EstimatorNotAvailable, effective)
        .expect("finite point estimate")
}

pub fn interval_cell(value: f64, low: f64, high: f64, effective: usize) -> GridCell {
    GridCell::with_interval(
        value,
        interval(low, high, ClusteringUnit::ParentWorld, effective),
        effective,
    )
    .expect("estimate inside its interval")
}

/// Builds a grid from `(capability, cell)` pairs under the given conditions.
pub fn grid_of(
    label: &str,
    conditions: MeasurementConditions,
    cells: Vec<(&str, GridCell)>,
) -> CapabilityGrid {
    cells.into_iter().fold(
        CapabilityGrid::new(label, conditions),
        |grid, (id, cell)| grid.with_cell(cap(id), cell),
    )
}
