//! The grid a metric is computed over, and the bridge from `bioprism-atlas`.
//!
//! A [`CapabilityGrid`] is one system's standing across a set of capabilities, measured under one
//! set of [`MeasurementConditions`]. It is the input to every aggregation, comparison, ranking and
//! gate in this crate, and it exists as a distinct type from `bioprism_atlas::Atlas` for one
//! reason: an atlas is a record of evidence, and a grid is a *reading* of that record taken under
//! stated conditions. Two grids can be built from one atlas — one restricted to a pack, one to a
//! site — and they are not comparable to each other.
//!
//! # The rule this type inherits
//!
//! `bioprism-atlas` holds that a capability with no evidence is `Unmeasured`, categorically
//! distinct from measured-and-poor, and that there is no `score_or_zero`. [`GridCell`] extends the
//! same rule upward: the unmeasured arm carries a `bioprism_atlas::UnmeasuredReason` — the same
//! vocabulary, imported rather than recreated — and no numeric payload. A renderer, an aggregator
//! or a ranker that matches on a hole has nothing to draw, average or compare.
//!
//! # What the atlas bridge does and does not claim
//!
//! [`CapabilityGrid::from_atlas`] reads every cell of an atlas. Measured cells become
//! [`Estimate::Point`] with [`NoIntervalReason::EstimatorNotAvailable`] and the measurement's
//! effective size — never an interval, because `bioprism-atlas` computes none and this crate will
//! not manufacture one from aggregate counts. That is not a limitation to work around; an interval
//! derived from pass and fail counts clusters at the trial, and 33.01 requires clustering at the
//! highest dependency level. The bridge therefore produces grids that a
//! [`crate::gate::GatePredicate::MaximumIntervalWidth`] gate reports as **unevaluable**, which is
//! the truthful verdict on an atlas that never estimated uncertainty.
//!
//! # Not implemented
//!
//! No evaluation execution: nothing here runs a trial, and no cell is ever produced by measuring
//! anything. No estimator. No store: a grid is built in memory from an atlas or from cells the
//! caller supplies, and persisting it is the hub's concern.

use crate::conditions::{MeasurementConditions, ScoringRule, Subject};
use crate::error::MetricsError;
use crate::interval::{Estimate, Interval, NoIntervalReason};
use bioprism_atlas::{Atlas, CapabilityId, UnmeasuredReason};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One cell: a measured estimate, or a stated hole.
///
/// Internally tagged on `state`, so the unmeasured form serializes as
/// `{"state":"unmeasured","reason":"not_attempted"}` with no numeric key anywhere in the object.
/// This mirrors `bioprism_atlas::CapabilityCell` exactly, on purpose: a consumer that has learned
/// to read one reads the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GridCell {
    Measured {
        estimate: Estimate,
        /// Independent clustering units behind the number, not the instance count. Carried per
        /// cell because a grid routinely mixes a capability measured across thirty parent worlds
        /// with one measured across two.
        effective_size: usize,
    },
    Unmeasured {
        reason: UnmeasuredReason,
    },
}

impl GridCell {
    pub fn measured(estimate: Estimate, effective_size: usize) -> Self {
        GridCell::Measured {
            estimate,
            effective_size,
        }
    }

    /// A cell measured as a bare number, with the reason it has no interval.
    pub fn point(
        value: f64,
        no_interval: NoIntervalReason,
        effective_size: usize,
    ) -> Result<Self, MetricsError> {
        Ok(GridCell::measured(
            Estimate::point(value, no_interval)?,
            effective_size,
        ))
    }

    pub fn with_interval(
        value: f64,
        interval: Interval,
        effective_size: usize,
    ) -> Result<Self, MetricsError> {
        Ok(GridCell::measured(
            Estimate::with_interval(value, interval)?,
            effective_size,
        ))
    }

    pub fn unmeasured(reason: UnmeasuredReason) -> Self {
        GridCell::Unmeasured { reason }
    }

    pub fn is_measured(&self) -> bool {
        matches!(self, GridCell::Measured { .. })
    }

    pub fn estimate(&self) -> Option<&Estimate> {
        match self {
            GridCell::Measured { estimate, .. } => Some(estimate),
            GridCell::Unmeasured { .. } => None,
        }
    }

    /// `None` for a hole. There is no `value_or_zero`, here or anywhere downstream of here.
    pub fn value(&self) -> Option<f64> {
        self.estimate().map(Estimate::value)
    }

    pub fn interval(&self) -> Option<&Interval> {
        self.estimate().and_then(Estimate::interval)
    }

    pub fn effective_size(&self) -> Option<usize> {
        match self {
            GridCell::Measured { effective_size, .. } => Some(*effective_size),
            GridCell::Unmeasured { .. } => None,
        }
    }

    pub fn unmeasured_reason(&self) -> Option<UnmeasuredReason> {
        match self {
            GridCell::Unmeasured { reason } => Some(*reason),
            GridCell::Measured { .. } => None,
        }
    }

    /// Whether a hole in this cell is one the declared intended use closes.
    ///
    /// Delegates to `bioprism_atlas::UnmeasuredReason::supports_claim`, which in turn delegates to
    /// the omission-manifest vocabulary of 43.26. One predicate, three crates.
    pub fn hole_is_closed_by_declaration(&self) -> bool {
        self.unmeasured_reason()
            .is_some_and(UnmeasuredReason::supports_claim)
    }

    pub fn validate(&self) -> Result<(), MetricsError> {
        if let GridCell::Measured { effective_size, .. } = self {
            if *effective_size == 0 {
                return Err(MetricsError::MalformedGrid {
                    detail: "a measured cell must have a positive effective size".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// One system's capability grid, under one set of conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityGrid {
    pub label: String,
    pub conditions: MeasurementConditions,
    cells: BTreeMap<CapabilityId, GridCell>,
}

impl CapabilityGrid {
    /// An empty grid under stated conditions. Aggregating over it refuses; that is correct.
    pub fn new(label: impl Into<String>, conditions: MeasurementConditions) -> Self {
        let label = label.into();
        CapabilityGrid {
            conditions: conditions.about(Subject::grid(label.clone())),
            label,
            cells: BTreeMap::new(),
        }
    }

    pub fn with_cell(mut self, capability: CapabilityId, cell: GridCell) -> Self {
        self.cells.insert(capability, cell);
        self
    }

    /// Reads an atlas into a grid under stated conditions.
    ///
    /// The ontology version is taken from the atlas rather than from the caller, because it is a
    /// fact about the evidence, not a choice. The scoring rule is fixed to
    /// [`ScoringRule::atlas_pass_rate`] for the same reason: it is what
    /// `bioprism_atlas::Measurement::score` computes, and letting a caller relabel it would break
    /// the one guarantee the label exists to give.
    pub fn from_atlas(
        label: impl Into<String>,
        atlas: &Atlas,
        conditions: MeasurementConditions,
    ) -> Self {
        let label = label.into();
        let conditions = MeasurementConditions {
            scoring_rule: ScoringRule::atlas_pass_rate(),
            ontology_version: crate::conditions::Condition::recorded(
                atlas.ontology().version().to_string(),
            ),
            ..conditions.about(Subject::grid(label.clone()))
        };
        let mut cells = BTreeMap::new();
        for (capability, cell) in atlas.cells() {
            let grid_cell = match cell.measurement() {
                Some(measurement) => {
                    let reason = if measurement.evaluable() == 1 {
                        NoIntervalReason::SingleTrial
                    } else {
                        NoIntervalReason::EstimatorNotAvailable
                    };
                    match Estimate::point(measurement.score(), reason) {
                        Ok(estimate) => GridCell::measured(estimate, measurement.effective_size()),
                        Err(_) => GridCell::unmeasured(UnmeasuredReason::NoEligibleEvidence),
                    }
                }
                None => GridCell::unmeasured(
                    cell.unmeasured_reason()
                        .unwrap_or(UnmeasuredReason::NotAttempted),
                ),
            };
            cells.insert(capability.clone(), grid_cell);
        }
        CapabilityGrid {
            label,
            conditions,
            cells,
        }
    }

    pub fn cell(&self, capability: &CapabilityId) -> Option<&GridCell> {
        self.cells.get(capability)
    }

    pub fn cells(&self) -> impl Iterator<Item = (&CapabilityId, &GridCell)> {
        self.cells.iter()
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &CapabilityId> {
        self.cells.keys()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn validate(&self) -> Result<(), MetricsError> {
        if self.label.trim().is_empty() {
            return Err(MetricsError::MalformedGrid {
                detail: "grid label must not be empty".to_string(),
            });
        }
        self.conditions
            .validate()
            .map_err(|detail| MetricsError::MalformedGrid {
                detail: format!("grid conditions are invalid: {detail}"),
            })?;
        match &self.conditions.subject {
            Subject::Grid { label } if label == &self.label => {}
            subject => {
                return Err(MetricsError::MalformedGrid {
                    detail: format!(
                        "grid label {} does not match its subject {}",
                        self.label, subject
                    ),
                })
            }
        }
        for (capability, cell) in &self.cells {
            cell.validate()
                .map_err(|error| MetricsError::MalformedGrid {
                    detail: format!("cell {capability}: {error}"),
                })?;
        }
        Ok(())
    }

    pub fn measured(&self) -> impl Iterator<Item = (&CapabilityId, &GridCell)> {
        self.cells.iter().filter(|(_, cell)| cell.is_measured())
    }

    /// The holes, with their reasons. Present for the same reason
    /// `bioprism_atlas::CoverageReport::holes` is never elided.
    pub fn holes(&self) -> impl Iterator<Item = (&CapabilityId, UnmeasuredReason)> {
        self.cells
            .iter()
            .filter_map(|(id, cell)| cell.unmeasured_reason().map(|reason| (id, reason)))
    }

    /// Restricts the grid to a capability subset, producing a *different* grid with a different
    /// label — never a mutation in place.
    ///
    /// A relabel is mandatory because a restricted grid is a different subject, and
    /// [`crate::comparability`] blocks a comparison between subjects. Silently keeping the label
    /// would let a pack-restricted reading masquerade as the whole.
    pub fn restricted_to(
        &self,
        label: impl Into<String>,
        capabilities: &[CapabilityId],
    ) -> CapabilityGrid {
        let label = label.into();
        let cells = capabilities
            .iter()
            .filter_map(|capability| {
                self.cells
                    .get(capability)
                    .map(|cell| (capability.clone(), cell.clone()))
            })
            .collect();
        CapabilityGrid {
            conditions: self.conditions.about(Subject::grid(label.clone())),
            label,
            cells,
        }
    }
}
