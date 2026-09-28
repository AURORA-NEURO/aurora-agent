//! Replicate-level decomposition of glioma population-state changes across tracked lineages.
//!
//! A symmetric two-factor decomposition separates relative lineage-abundance shifts from
//! within-lineage state-composition shifts. Bootstrap resampling is at the independent experimental
//! unit level, never at the barcode or time-window level. The analysis is descriptive: a lineage
//! shift does not prove a genetic cause, and within-lineage redistribution is not proof of
//! individual-cell switching. STAG-style transition/growth models need richer data and assumptions.
//! Implements `GAF-GLIOMA-P10-F32`.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaLineageDynamics1@2";
pub const SHARE_SCALE: i64 = 1_000_000;
pub const MAX_STATES: usize = 32;
pub const MAX_UNITS_PER_ARM: usize = 128;
pub const MAX_LINEAGES_PER_UNIT: usize = 4_096;
pub const MAX_SNAPSHOTS: usize = 65_536;
pub const MAX_CELLS_PER_STATE: u64 = 1_000_000_000_000;
pub const MIN_BOOTSTRAP_REPLICATES: usize = 999;
pub const MAX_BOOTSTRAP_REPLICATES: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageStateCount {
    pub state_id: String,
    /// Zero is an observed nondetection only when this complete snapshot exists.
    pub cell_count: u64,
}

/// Complete state counts for one barcode lineage in one independent preclinical replicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageStateSnapshot {
    pub observation_id: String,
    /// Independent organoid, animal, culture, or other biological replicate.
    pub experimental_unit_id: String,
    /// A tracked lineage nested within an experimental unit, never an independent replicate.
    pub lineage_id: String,
    pub arm_id: String,
    pub batch_id: String,
    pub model_system: GliomaModelSystem,
    pub timepoint: u32,
    /// Must contain every declared state in request order; use explicit zero counts for nondetection.
    pub state_counts: Vec<LineageStateCount>,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageDynamicsRequest {
    pub objective: String,
    pub control_arm: String,
    pub comparison_arm: String,
    pub model_system: GliomaModelSystem,
    pub state_order: Vec<String>,
    pub baseline_timepoint: u32,
    pub followup_timepoint: u32,
    pub min_units_per_arm: usize,
    pub min_lineages_per_unit: usize,
    pub min_cells_per_unit_timepoint: u64,
    /// Minimum absolute descriptive between-arm component difference, in parts per million.
    pub minimum_difference_ppm: u32,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitEligibility {
    Included,
    InsufficientLineages,
    InsufficientCells,
    NoObservedCellsAtTimepoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitStateDynamics {
    pub state_id: String,
    /// Exact counts keep downstream support gates independent of rounded shares.
    pub baseline_cell_count: u64,
    pub followup_cell_count: u64,
    pub baseline_share_ppm: i32,
    pub followup_share_ppm: i32,
    pub population_shift_ppm: i32,
    pub lineage_selection_component_ppm: i32,
    pub within_lineage_redistribution_component_ppm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageUnitDynamics {
    pub experimental_unit_id: String,
    pub arm_id: String,
    pub lineage_count: usize,
    pub baseline_cell_count: u64,
    pub followup_cell_count: u64,
    pub eligibility: UnitEligibility,
    pub states: Vec<UnitStateDynamics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmStateDynamics {
    pub baseline_share_ppm: i32,
    pub followup_share_ppm: i32,
    pub population_shift_ppm: i32,
    pub lineage_selection_component_ppm: i32,
    pub within_lineage_redistribution_component_ppm: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapInterval {
    pub lower_ppm: i32,
    pub upper_ppm: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCall {
    DifferenceBeyondMargin,
    WithinNullMargin,
    Inconclusive,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateLineageDynamicsContrast {
    pub state_id: String,
    pub control: ArmStateDynamics,
    pub comparison: ArmStateDynamics,
    pub population_shift_difference_ppm: i32,
    pub population_shift_interval: Option<BootstrapInterval>,
    pub population_shift_call: ComponentCall,
    pub lineage_selection_difference_ppm: i32,
    pub lineage_selection_interval: Option<BootstrapInterval>,
    pub lineage_selection_call: ComponentCall,
    pub within_lineage_redistribution_difference_ppm: i32,
    pub within_lineage_redistribution_interval: Option<BootstrapInterval>,
    pub within_lineage_redistribution_call: ComponentCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageDynamicsDisposition {
    BetweenArmDifferenceSupported,
    NoBetweenArmDifferenceWithinMargin,
    Inconclusive,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageDynamicsAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_arm: String,
    pub comparison_arm: String,
    pub state_order: Vec<String>,
    pub baseline_timepoint: u32,
    pub followup_timepoint: u32,
    pub included_unit_order: Vec<String>,
    pub excluded_unit_order: Vec<String>,
    pub units: Vec<LineageUnitDynamics>,
    pub contrasts: Vec<StateLineageDynamicsContrast>,
    pub minimum_difference_ppm: u32,
    pub min_units_per_arm: usize,
    pub min_lineages_per_unit: usize,
    pub min_cells_per_unit_timepoint: u64,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: ContentHash,
    pub confidence_level_milli: u16,
    pub disposition: LineageDynamicsDisposition,
    pub limitations: Vec<String>,
    pub input_digest: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineageDynamicsError {
    #[error("lineage dynamics request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage dynamics input is invalid: {0}")]
    InvalidInput(String),
    #[error("lineage dynamics output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage dynamics digest failed: {0}")]
    Digest(String),
}

impl LineageDynamicsAnalysis {
    pub fn validate(&self) -> Result<(), LineageDynamicsError> {
        let ordered_units = self.units.windows(2).all(|pair| {
            (&pair[0].arm_id, &pair[0].experimental_unit_id)
                < (&pair[1].arm_id, &pair[1].experimental_unit_id)
        });
        let unique_unit_ids = self
            .units
            .iter()
            .map(|unit| unit.experimental_unit_id.as_str())
            .collect::<BTreeSet<_>>();
        let expected_included = self
            .units
            .iter()
            .filter(|unit| unit.eligibility == UnitEligibility::Included)
            .map(|unit| unit.experimental_unit_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_excluded = self
            .units
            .iter()
            .filter(|unit| unit.eligibility != UnitEligibility::Included)
            .map(|unit| unit.experimental_unit_id.clone())
            .collect::<BTreeSet<_>>();
        let matches_order = |actual: &[String], expected: &BTreeSet<String>| {
            actual.windows(2).all(|pair| pair[0] < pair[1])
                && actual.iter().cloned().collect::<BTreeSet<_>>() == *expected
        };
        let included = self
            .units
            .iter()
            .filter(|unit| unit.eligibility == UnitEligibility::Included)
            .collect::<Vec<_>>();
        let enough_units = [self.control_arm.as_str(), self.comparison_arm.as_str()]
            .into_iter()
            .all(|arm| {
                included.iter().filter(|unit| unit.arm_id == arm).count() >= self.min_units_per_arm
            });
        let valid_units = self.units.iter().all(|unit| {
            let arm_is_valid =
                unit.arm_id == self.control_arm || unit.arm_id == self.comparison_arm;
            let observed_at_both = unit.baseline_cell_count > 0 && unit.followup_cell_count > 0;
            let expected_eligibility = if !observed_at_both {
                UnitEligibility::NoObservedCellsAtTimepoint
            } else if unit.lineage_count < self.min_lineages_per_unit {
                UnitEligibility::InsufficientLineages
            } else if unit.baseline_cell_count < self.min_cells_per_unit_timepoint
                || unit.followup_cell_count < self.min_cells_per_unit_timepoint
            {
                UnitEligibility::InsufficientCells
            } else {
                UnitEligibility::Included
            };
            arm_is_valid
                && !unit.experimental_unit_id.trim().is_empty()
                && unit.lineage_count > 0
                && unit.lineage_count <= MAX_LINEAGES_PER_UNIT
                && unit.baseline_cell_count
                    <= MAX_CELLS_PER_STATE * MAX_STATES as u64 * MAX_LINEAGES_PER_UNIT as u64
                && unit.followup_cell_count
                    <= MAX_CELLS_PER_STATE * MAX_STATES as u64 * MAX_LINEAGES_PER_UNIT as u64
                && unit.eligibility == expected_eligibility
                && unit.states.len()
                    == if observed_at_both {
                        self.state_order.len()
                    } else {
                        0
                    }
                && unit
                    .states
                    .iter()
                    .zip(&self.state_order)
                    .all(|(state, state_id)| {
                        state.state_id == *state_id
                            && state.baseline_cell_count
                                <= MAX_CELLS_PER_STATE * MAX_LINEAGES_PER_UNIT as u64
                            && state.followup_cell_count
                                <= MAX_CELLS_PER_STATE * MAX_LINEAGES_PER_UNIT as u64
                            && (0..=SHARE_SCALE as i32).contains(&state.baseline_share_ppm)
                            && (0..=SHARE_SCALE as i32).contains(&state.followup_share_ppm)
                            && (-SHARE_SCALE as i32..=SHARE_SCALE as i32)
                                .contains(&state.lineage_selection_component_ppm)
                            && (-SHARE_SCALE as i32..=SHARE_SCALE as i32)
                                .contains(&state.within_lineage_redistribution_component_ppm)
                            && state.population_shift_ppm
                                == state.followup_share_ppm - state.baseline_share_ppm
                            && state.population_shift_ppm
                                == state.lineage_selection_component_ppm
                                    + state.within_lineage_redistribution_component_ppm
                    })
                && (unit.states.is_empty() || {
                    let baseline_counts = unit
                        .states
                        .iter()
                        .map(|state| state.baseline_cell_count)
                        .collect::<Vec<_>>();
                    let followup_counts = unit
                        .states
                        .iter()
                        .map(|state| state.followup_cell_count)
                        .collect::<Vec<_>>();
                    unit.states
                        .iter()
                        .map(|state| state.baseline_share_ppm)
                        .sum::<i32>()
                        == SHARE_SCALE as i32
                        && unit
                            .states
                            .iter()
                            .map(|state| state.followup_share_ppm)
                            .sum::<i32>()
                            == SHARE_SCALE as i32
                        && unit
                            .states
                            .iter()
                            .map(|state| state.population_shift_ppm)
                            .sum::<i32>()
                            == 0
                        && baseline_counts.iter().sum::<u64>() == unit.baseline_cell_count
                        && followup_counts.iter().sum::<u64>() == unit.followup_cell_count
                        && shares_ppm(&baseline_counts)
                            .iter()
                            .map(|share| *share as i32)
                            .eq(unit.states.iter().map(|state| state.baseline_share_ppm))
                        && shares_ppm(&followup_counts)
                            .iter()
                            .map(|share| *share as i32)
                            .eq(unit.states.iter().map(|state| state.followup_share_ppm))
                })
        });
        let valid_contrasts = self.contrasts.len() == self.state_order.len()
            && self
                .contrasts
                .iter()
                .zip(&self.state_order)
                .all(|(contrast, state_id)| {
                    let summaries = [&contrast.control, &contrast.comparison];
                    let intervals = [
                        contrast.population_shift_interval,
                        contrast.lineage_selection_interval,
                        contrast.within_lineage_redistribution_interval,
                    ];
                    let actual_calls = [
                        contrast.population_shift_call,
                        contrast.lineage_selection_call,
                        contrast.within_lineage_redistribution_call,
                    ];
                    let differences = [
                        contrast.population_shift_difference_ppm,
                        contrast.lineage_selection_difference_ppm,
                        contrast.within_lineage_redistribution_difference_ppm,
                    ];
                    contrast.state_id == *state_id
                        && summaries.iter().all(|summary| {
                            (0..=SHARE_SCALE as i32).contains(&summary.baseline_share_ppm)
                                && (0..=SHARE_SCALE as i32).contains(&summary.followup_share_ppm)
                                && (-SHARE_SCALE as i32..=SHARE_SCALE as i32)
                                    .contains(&summary.population_shift_ppm)
                                && (-SHARE_SCALE as i32..=SHARE_SCALE as i32)
                                    .contains(&summary.lineage_selection_component_ppm)
                                && (-SHARE_SCALE as i32..=SHARE_SCALE as i32)
                                    .contains(&summary.within_lineage_redistribution_component_ppm)
                                && (summary.population_shift_ppm
                                    - summary.lineage_selection_component_ppm
                                    - summary.within_lineage_redistribution_component_ppm)
                                    .abs()
                                    <= 2
                        })
                        && contrast.population_shift_difference_ppm
                            == contrast.comparison.population_shift_ppm
                                - contrast.control.population_shift_ppm
                        && contrast.lineage_selection_difference_ppm
                            == contrast.comparison.lineage_selection_component_ppm
                                - contrast.control.lineage_selection_component_ppm
                        && contrast.within_lineage_redistribution_difference_ppm
                            == contrast
                                .comparison
                                .within_lineage_redistribution_component_ppm
                                - contrast.control.within_lineage_redistribution_component_ppm
                        && differences.iter().all(|value| {
                            (-2 * SHARE_SCALE as i32..=2 * SHARE_SCALE as i32).contains(value)
                        })
                        && intervals
                            .iter()
                            .all(|interval| interval.is_some() == enough_units)
                        && intervals.iter().flatten().all(|interval| {
                            interval.lower_ppm <= interval.upper_ppm
                                && (-2 * SHARE_SCALE as i32..=2 * SHARE_SCALE as i32)
                                    .contains(&interval.lower_ppm)
                                && (-2 * SHARE_SCALE as i32..=2 * SHARE_SCALE as i32)
                                    .contains(&interval.upper_ppm)
                        })
                        && if enough_units {
                            intervals.iter().zip(actual_calls).all(|(interval, call)| {
                                interval.is_some_and(|value| {
                                    call == call_interval(&value, self.minimum_difference_ppm)
                                })
                            })
                        } else {
                            actual_calls
                                .iter()
                                .all(|call| *call == ComponentCall::Unresolved)
                        }
                });
        let expected_disposition = disposition_from_calls(enough_units, &self.contrasts);
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LineageDynamicsError::Digest(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.comparison_arm.trim().is_empty()
            || self.control_arm == self.comparison_arm
            || self.state_order.len() < 2
            || self.state_order.len() > MAX_STATES
            || self.state_order.iter().any(|state| state.trim().is_empty())
            || self.state_order.iter().collect::<BTreeSet<_>>().len() != self.state_order.len()
            || self.baseline_timepoint >= self.followup_timepoint
            || !(2..=MAX_UNITS_PER_ARM).contains(&self.min_units_per_arm)
            || !(2..=MAX_LINEAGES_PER_UNIT).contains(&self.min_lineages_per_unit)
            || self.min_cells_per_unit_timepoint == 0
            || !(MIN_BOOTSTRAP_REPLICATES..=MAX_BOOTSTRAP_REPLICATES)
                .contains(&self.bootstrap_replicates)
            || self.minimum_difference_ppm == 0
            || self.minimum_difference_ppm > SHARE_SCALE as u32
            || self.confidence_level_milli != 950
            || !ordered_units
            || unique_unit_ids.len() != self.units.len()
            || self.units.len() > MAX_UNITS_PER_ARM * 2
            || !matches_order(&self.included_unit_order, &expected_included)
            || !matches_order(&self.excluded_unit_order, &expected_excluded)
            || !expected_included.is_disjoint(&expected_excluded)
            || !valid_units
            || !valid_contrasts
            || self.limitations.is_empty()
            || self.limitations.windows(2).any(|pair| pair[0] >= pair[1])
            || self.limitations.iter().any(|item| item.trim().is_empty())
            || self.disposition != expected_disposition
            || self.input_digest.as_str().len() != 64
            || self.digest.as_str().len() != 64
            || self.digest != expected_digest
        {
            return Err(LineageDynamicsError::InvalidOutput(
                "analysis identity, replicate ordering, eligibility, component reconciliation, intervals, limitations, or digest is invalid".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn from_hash(hash: &ContentHash) -> Self {
        let seed = u64::from_str_radix(&hash.as_str()[..16], 16).unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() % length as u64) as usize
    }
}

fn digest_input(output: &LineageDynamicsAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "control_arm": output.control_arm,
        "comparison_arm": output.comparison_arm,
        "state_order": output.state_order,
        "baseline_timepoint": output.baseline_timepoint,
        "followup_timepoint": output.followup_timepoint,
        "included_unit_order": output.included_unit_order,
        "excluded_unit_order": output.excluded_unit_order,
        "units": output.units,
        "contrasts": output.contrasts,
        "minimum_difference_ppm": output.minimum_difference_ppm,
        "min_units_per_arm": output.min_units_per_arm,
        "min_lineages_per_unit": output.min_lineages_per_unit,
        "min_cells_per_unit_timepoint": output.min_cells_per_unit_timepoint,
        "bootstrap_replicates": output.bootstrap_replicates,
        "bootstrap_seed": output.bootstrap_seed,
        "confidence_level_milli": output.confidence_level_milli,
        "disposition": output.disposition,
        "limitations": output.limitations,
        "input_digest": output.input_digest,
    })
}

fn validate_request(request: &LineageDynamicsRequest) -> Result<(), LineageDynamicsError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.comparison_arm.trim().is_empty()
        || request.control_arm == request.comparison_arm
        || request.state_order.len() < 2
        || request.state_order.len() > MAX_STATES
        || request
            .state_order
            .iter()
            .any(|state| state.trim().is_empty())
        || request.state_order.iter().collect::<BTreeSet<_>>().len() != request.state_order.len()
        || request.baseline_timepoint >= request.followup_timepoint
        || request.min_units_per_arm < 2
        || request.min_units_per_arm > MAX_UNITS_PER_ARM
        || request.min_lineages_per_unit < 2
        || request.min_lineages_per_unit > MAX_LINEAGES_PER_UNIT
        || request.min_cells_per_unit_timepoint == 0
        || request.minimum_difference_ppm == 0
        || request.minimum_difference_ppm > SHARE_SCALE as u32
        || !(MIN_BOOTSTRAP_REPLICATES..=MAX_BOOTSTRAP_REPLICATES)
            .contains(&request.bootstrap_replicates)
    {
        return Err(LineageDynamicsError::InvalidRequest(
            "objective, arms, states, paired timepoints, independent-unit/lineage/cell floors, effect margin, or bootstrap bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_snapshots(
    request: &LineageDynamicsRequest,
    snapshots: &[LineageStateSnapshot],
) -> Result<Vec<LineageStateSnapshot>, LineageDynamicsError> {
    if snapshots.is_empty() || snapshots.len() > MAX_SNAPSHOTS {
        return Err(LineageDynamicsError::InvalidInput(
            "lineage snapshot count is empty or exceeds the bounded capacity".into(),
        ));
    }
    let arms = BTreeSet::from([
        request.control_arm.as_str(),
        request.comparison_arm.as_str(),
    ]);
    let mut observations = BTreeSet::new();
    let mut unit_arm = BTreeMap::<String, String>::new();
    let mut keys = BTreeSet::new();
    for snapshot in snapshots {
        if snapshot.observation_id.trim().is_empty()
            || snapshot.experimental_unit_id.trim().is_empty()
            || snapshot.lineage_id.trim().is_empty()
            || snapshot.batch_id.trim().is_empty()
            || !arms.contains(snapshot.arm_id.as_str())
            || snapshot.model_system != request.model_system
            || ![request.baseline_timepoint, request.followup_timepoint]
                .contains(&snapshot.timepoint)
            || !observations.insert(snapshot.observation_id.clone())
            || !keys.insert((
                snapshot.arm_id.clone(),
                snapshot.experimental_unit_id.clone(),
                snapshot.lineage_id.clone(),
                snapshot.timepoint,
            ))
            || snapshot.state_counts.len() != request.state_order.len()
            || snapshot
                .state_counts
                .iter()
                .zip(&request.state_order)
                .any(|(count, state)| {
                    count.state_id != *state || count.cell_count > MAX_CELLS_PER_STATE
                })
        {
            return Err(LineageDynamicsError::InvalidInput(
                "snapshot identity, arm/model/timepoint, uniqueness, full state vector, or cell-count bound is invalid".into(),
            ));
        }
        if unit_arm
            .insert(
                snapshot.experimental_unit_id.clone(),
                snapshot.arm_id.clone(),
            )
            .is_some_and(|arm| arm != snapshot.arm_id)
        {
            return Err(LineageDynamicsError::InvalidInput(format!(
                "experimental unit {} appears in multiple arms",
                snapshot.experimental_unit_id
            )));
        }
        snapshot
            .artifact
            .validate()
            .map_err(|error| LineageDynamicsError::InvalidInput(error.to_string()))?;
        if !snapshot.artifact.local_only
            || snapshot.artifact.contains_human_data
            || snapshot.artifact.contains_direct_identifiers
        {
            return Err(LineageDynamicsError::InvalidInput(
                "analysis accepts only local preclinical artifacts without human data or direct identifiers".into(),
            ));
        }
    }
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|left, right| {
        left.arm_id
            .cmp(&right.arm_id)
            .then_with(|| left.experimental_unit_id.cmp(&right.experimental_unit_id))
            .then_with(|| left.lineage_id.cmp(&right.lineage_id))
            .then_with(|| left.timepoint.cmp(&right.timepoint))
            .then_with(|| left.observation_id.cmp(&right.observation_id))
    });
    Ok(sorted)
}

fn round_div(numerator: i128, denominator: i128) -> i64 {
    if numerator >= 0 {
        ((numerator + denominator / 2) / denominator) as i64
    } else {
        -(((-numerator + denominator / 2) / denominator) as i64)
    }
}

/// Largest-remainder normalization makes a composition sum exactly to one million ppm.
fn shares_ppm(counts: &[u64]) -> Vec<i64> {
    let total = counts.iter().map(|count| u128::from(*count)).sum::<u128>();
    if total == 0 {
        return vec![0; counts.len()];
    }
    let mut shares = Vec::with_capacity(counts.len());
    let mut remainders = Vec::with_capacity(counts.len());
    let mut assigned = 0_i64;
    for (index, count) in counts.iter().enumerate() {
        let scaled = u128::from(*count) * SHARE_SCALE as u128;
        let share = (scaled / total) as i64;
        assigned += share;
        shares.push(share);
        remainders.push((index, scaled % total));
    }
    remainders.sort_by(|(left_index, left), (right_index, right)| {
        right.cmp(left).then_with(|| left_index.cmp(right_index))
    });
    for (index, _) in remainders
        .into_iter()
        .take((SHARE_SCALE - assigned) as usize)
    {
        shares[index] += 1;
    }
    shares
}

fn total(counts: &[u64]) -> u64 {
    counts.iter().sum()
}

fn mean(values: &[i32]) -> i32 {
    if values.is_empty() {
        return 0;
    }
    round_div(
        values.iter().map(|value| i128::from(*value)).sum(),
        values.len() as i128,
    ) as i32
}

fn decompose_unit(
    request: &LineageDynamicsRequest,
    lineages: &BTreeMap<String, [Vec<u64>; 2]>,
) -> Result<(u64, u64, Vec<UnitStateDynamics>), LineageDynamicsError> {
    let state_count = request.state_order.len();
    let mut baseline_population = vec![0_u64; state_count];
    let mut followup_population = vec![0_u64; state_count];
    let mut baseline_lineage_totals = Vec::with_capacity(lineages.len());
    let mut followup_lineage_totals = Vec::with_capacity(lineages.len());
    let mut baseline_total = 0_u64;
    let mut followup_total = 0_u64;
    for snapshots in lineages.values() {
        let base = total(&snapshots[0]);
        let follow = total(&snapshots[1]);
        baseline_total = baseline_total.checked_add(base).ok_or_else(|| {
            LineageDynamicsError::InvalidInput("baseline cell total overflowed".into())
        })?;
        followup_total = followup_total.checked_add(follow).ok_or_else(|| {
            LineageDynamicsError::InvalidInput("follow-up cell total overflowed".into())
        })?;
        for state_index in 0..state_count {
            baseline_population[state_index] = baseline_population[state_index]
                .checked_add(snapshots[0][state_index])
                .ok_or_else(|| {
                    LineageDynamicsError::InvalidInput("baseline state total overflowed".into())
                })?;
            followup_population[state_index] = followup_population[state_index]
                .checked_add(snapshots[1][state_index])
                .ok_or_else(|| {
                    LineageDynamicsError::InvalidInput("follow-up state total overflowed".into())
                })?;
        }
        baseline_lineage_totals.push(base);
        followup_lineage_totals.push(follow);
    }
    if baseline_total == 0 || followup_total == 0 {
        return Ok((baseline_total, followup_total, Vec::new()));
    }

    let base_state_share = shares_ppm(&baseline_population);
    let follow_state_share = shares_ppm(&followup_population);
    let base_lineage_share = shares_ppm(&baseline_lineage_totals);
    let follow_lineage_share = shares_ppm(&followup_lineage_totals);
    let mut selection_numerators = vec![0_i128; state_count];
    let mut redistribution_numerators = vec![0_i128; state_count];
    for (lineage_index, snapshots) in lineages.values().enumerate() {
        let mut base_q = shares_ppm(&snapshots[0]);
        let mut follow_q = shares_ppm(&snapshots[1]);
        if baseline_lineage_totals[lineage_index] == 0 {
            base_q.clone_from(&follow_q);
        }
        if followup_lineage_totals[lineage_index] == 0 {
            follow_q.clone_from(&base_q);
        }
        let base_weight = base_lineage_share[lineage_index];
        let follow_weight = follow_lineage_share[lineage_index];
        for state_index in 0..state_count {
            selection_numerators[state_index] += i128::from(follow_weight - base_weight)
                * i128::from(follow_q[state_index] + base_q[state_index]);
            redistribution_numerators[state_index] +=
                i128::from(follow_q[state_index] - base_q[state_index])
                    * i128::from(follow_weight + base_weight);
        }
    }

    let denominator = i128::from(2 * SHARE_SCALE);
    let mut states = Vec::with_capacity(state_count);
    for state_index in 0..state_count {
        let shift = follow_state_share[state_index] - base_state_share[state_index];
        let selection = round_div(selection_numerators[state_index], denominator);
        let redistribution = shift - selection;
        let formula_redistribution = round_div(redistribution_numerators[state_index], denominator);
        if (redistribution - formula_redistribution).abs() > lineages.len() as i64 + 2 {
            return Err(LineageDynamicsError::InvalidOutput(
                "symmetric lineage decomposition exceeded its integer reconciliation bound".into(),
            ));
        }
        states.push(UnitStateDynamics {
            state_id: request.state_order[state_index].clone(),
            baseline_cell_count: baseline_population[state_index],
            followup_cell_count: followup_population[state_index],
            baseline_share_ppm: base_state_share[state_index] as i32,
            followup_share_ppm: follow_state_share[state_index] as i32,
            population_shift_ppm: shift as i32,
            lineage_selection_component_ppm: selection as i32,
            within_lineage_redistribution_component_ppm: redistribution as i32,
        });
    }
    Ok((baseline_total, followup_total, states))
}

fn percentile_interval(
    control: &[i32],
    comparison: &[i32],
    replicates: usize,
    rng: &mut SplitMix64,
) -> BootstrapInterval {
    let mut samples = Vec::with_capacity(replicates);
    let mut control_sample = Vec::with_capacity(control.len());
    let mut comparison_sample = Vec::with_capacity(comparison.len());
    for _ in 0..replicates {
        control_sample.clear();
        comparison_sample.clear();
        for _ in 0..control.len() {
            control_sample.push(control[rng.index(control.len())]);
        }
        for _ in 0..comparison.len() {
            comparison_sample.push(comparison[rng.index(comparison.len())]);
        }
        samples.push(mean(&comparison_sample) - mean(&control_sample));
    }
    samples.sort_unstable();
    let last = replicates - 1;
    BootstrapInterval {
        lower_ppm: samples[(last * 25) / 1_000],
        upper_ppm: samples[((last * 975).div_ceil(1_000)).min(last)],
    }
}

fn call_interval(interval: &BootstrapInterval, margin_ppm: u32) -> ComponentCall {
    let margin = margin_ppm as i32;
    if interval.lower_ppm > margin || interval.upper_ppm < -margin {
        ComponentCall::DifferenceBeyondMargin
    } else if interval.lower_ppm >= -margin && interval.upper_ppm <= margin {
        ComponentCall::WithinNullMargin
    } else {
        ComponentCall::Inconclusive
    }
}

fn disposition_from_calls(
    enough_units: bool,
    contrasts: &[StateLineageDynamicsContrast],
) -> LineageDynamicsDisposition {
    if !enough_units {
        return LineageDynamicsDisposition::Unresolved;
    }
    let calls = contrasts
        .iter()
        .flat_map(|contrast| {
            [
                contrast.population_shift_call,
                contrast.lineage_selection_call,
                contrast.within_lineage_redistribution_call,
            ]
        })
        .collect::<Vec<_>>();
    if calls.contains(&ComponentCall::DifferenceBeyondMargin) {
        LineageDynamicsDisposition::BetweenArmDifferenceSupported
    } else if calls
        .iter()
        .all(|call| *call == ComponentCall::WithinNullMargin)
    {
        LineageDynamicsDisposition::NoBetweenArmDifferenceWithinMargin
    } else {
        LineageDynamicsDisposition::Inconclusive
    }
}

fn source_digest(
    request: &LineageDynamicsRequest,
    snapshots: &[LineageStateSnapshot],
) -> Result<ContentHash, LineageDynamicsError> {
    ContentHash::of_value(&serde_json::json!({"request": request, "snapshots": snapshots}))
        .map_err(|error| LineageDynamicsError::Digest(error.to_string()))
}

/// Decomposes between-lineage abundance and within-lineage composition shifts, resampling whole
/// experimental units for arm contrasts.
///
/// Every lineage must have complete state-count snapshots at both requested timepoints. An
/// explicit all-zero vector means an assayed nondetection; a missing snapshot is an input error.
pub fn analyze_glioma_lineage_dynamics(
    request: &LineageDynamicsRequest,
    snapshots: &[LineageStateSnapshot],
) -> Result<LineageDynamicsAnalysis, LineageDynamicsError> {
    validate_request(request)?;
    let snapshots = validate_snapshots(request, snapshots)?;
    let input_digest = source_digest(request, &snapshots)?;
    let mut lineage_pairs = BTreeMap::<(String, String, String), [Option<Vec<u64>>; 2]>::new();
    for snapshot in &snapshots {
        let time_index = usize::from(snapshot.timepoint == request.followup_timepoint);
        let key = (
            snapshot.arm_id.clone(),
            snapshot.experimental_unit_id.clone(),
            snapshot.lineage_id.clone(),
        );
        let state_counts = snapshot
            .state_counts
            .iter()
            .map(|count| count.cell_count)
            .collect::<Vec<_>>();
        let entry = lineage_pairs.entry(key).or_insert([None, None]);
        if entry[time_index].replace(state_counts).is_some() {
            return Err(LineageDynamicsError::InvalidInput(
                "duplicate lineage snapshot for a requested timepoint".into(),
            ));
        }
    }

    let mut by_unit = BTreeMap::<(String, String), BTreeMap<String, [Vec<u64>; 2]>>::new();
    for ((arm_id, unit_id, lineage_id), pair) in lineage_pairs {
        let [Some(baseline), Some(followup)] = pair else {
            return Err(LineageDynamicsError::InvalidInput(format!(
                "lineage {lineage_id} in unit {unit_id} needs explicit baseline and follow-up snapshots; use zero counts for assayed nondetection"
            )));
        };
        if total(&baseline) == 0 && total(&followup) == 0 {
            return Err(LineageDynamicsError::InvalidInput(format!(
                "lineage {lineage_id} has no cells at either timepoint and is not an observed lineage"
            )));
        }
        by_unit
            .entry((arm_id, unit_id))
            .or_default()
            .insert(lineage_id, [baseline, followup]);
    }
    if by_unit.len() > MAX_UNITS_PER_ARM * 2 {
        return Err(LineageDynamicsError::InvalidInput(
            "independent unit count exceeds the two-arm analysis limit".into(),
        ));
    }
    for arm in [&request.control_arm, &request.comparison_arm] {
        if by_unit
            .keys()
            .filter(|(unit_arm, _)| unit_arm == arm)
            .count()
            > MAX_UNITS_PER_ARM
        {
            return Err(LineageDynamicsError::InvalidInput(format!(
                "arm {arm} exceeds the independent-unit capacity"
            )));
        }
    }

    let mut units = Vec::with_capacity(by_unit.len());
    for ((arm_id, experimental_unit_id), lineages) in by_unit {
        if lineages.len() > MAX_LINEAGES_PER_UNIT {
            return Err(LineageDynamicsError::InvalidInput(format!(
                "experimental unit {experimental_unit_id} exceeds the lineage capacity"
            )));
        }
        let (baseline_cell_count, followup_cell_count, states) =
            decompose_unit(request, &lineages)?;
        let eligibility = if baseline_cell_count == 0 || followup_cell_count == 0 {
            UnitEligibility::NoObservedCellsAtTimepoint
        } else if lineages.len() < request.min_lineages_per_unit {
            UnitEligibility::InsufficientLineages
        } else if baseline_cell_count < request.min_cells_per_unit_timepoint
            || followup_cell_count < request.min_cells_per_unit_timepoint
        {
            UnitEligibility::InsufficientCells
        } else {
            UnitEligibility::Included
        };
        units.push(LineageUnitDynamics {
            experimental_unit_id,
            arm_id,
            lineage_count: lineages.len(),
            baseline_cell_count,
            followup_cell_count,
            eligibility,
            states,
        });
    }
    units.sort_by(|left, right| {
        left.arm_id
            .cmp(&right.arm_id)
            .then_with(|| left.experimental_unit_id.cmp(&right.experimental_unit_id))
    });
    let included = units
        .iter()
        .filter(|unit| unit.eligibility == UnitEligibility::Included)
        .collect::<Vec<_>>();
    let included_unit_order = included
        .iter()
        .map(|unit| unit.experimental_unit_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let excluded_unit_order = units
        .iter()
        .filter(|unit| unit.eligibility != UnitEligibility::Included)
        .map(|unit| unit.experimental_unit_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let enough_units = [
        request.control_arm.as_str(),
        request.comparison_arm.as_str(),
    ]
    .into_iter()
    .all(|arm| {
        included.iter().filter(|unit| unit.arm_id == arm).count() >= request.min_units_per_arm
    });

    let mut rng = SplitMix64::from_hash(&request.bootstrap_seed);
    let mut contrasts = Vec::with_capacity(request.state_order.len());
    for (state_index, state_id) in request.state_order.iter().enumerate() {
        let control = included
            .iter()
            .filter(|unit| unit.arm_id == request.control_arm)
            .map(|unit| &unit.states[state_index])
            .collect::<Vec<_>>();
        let comparison = included
            .iter()
            .filter(|unit| unit.arm_id == request.comparison_arm)
            .map(|unit| &unit.states[state_index])
            .collect::<Vec<_>>();
        let summarize = |rows: &[&UnitStateDynamics]| ArmStateDynamics {
            baseline_share_ppm: mean(
                &rows
                    .iter()
                    .map(|row| row.baseline_share_ppm)
                    .collect::<Vec<_>>(),
            ),
            followup_share_ppm: mean(
                &rows
                    .iter()
                    .map(|row| row.followup_share_ppm)
                    .collect::<Vec<_>>(),
            ),
            population_shift_ppm: mean(
                &rows
                    .iter()
                    .map(|row| row.population_shift_ppm)
                    .collect::<Vec<_>>(),
            ),
            lineage_selection_component_ppm: mean(
                &rows
                    .iter()
                    .map(|row| row.lineage_selection_component_ppm)
                    .collect::<Vec<_>>(),
            ),
            within_lineage_redistribution_component_ppm: mean(
                &rows
                    .iter()
                    .map(|row| row.within_lineage_redistribution_component_ppm)
                    .collect::<Vec<_>>(),
            ),
        };
        let control_summary = summarize(&control);
        let comparison_summary = summarize(&comparison);
        let population_difference =
            comparison_summary.population_shift_ppm - control_summary.population_shift_ppm;
        let selection_difference = comparison_summary.lineage_selection_component_ppm
            - control_summary.lineage_selection_component_ppm;
        let redistribution_difference = comparison_summary
            .within_lineage_redistribution_component_ppm
            - control_summary.within_lineage_redistribution_component_ppm;
        if enough_units {
            let component_values =
                |rows: &[&UnitStateDynamics], select: fn(&UnitStateDynamics) -> i32| {
                    rows.iter().map(|row| select(row)).collect::<Vec<_>>()
                };
            let control_population = component_values(&control, |row| row.population_shift_ppm);
            let comparison_population =
                component_values(&comparison, |row| row.population_shift_ppm);
            let control_selection =
                component_values(&control, |row| row.lineage_selection_component_ppm);
            let comparison_selection =
                component_values(&comparison, |row| row.lineage_selection_component_ppm);
            let control_redistribution = component_values(&control, |row| {
                row.within_lineage_redistribution_component_ppm
            });
            let comparison_redistribution = component_values(&comparison, |row| {
                row.within_lineage_redistribution_component_ppm
            });
            let population_interval = percentile_interval(
                &control_population,
                &comparison_population,
                request.bootstrap_replicates,
                &mut rng,
            );
            let selection_interval = percentile_interval(
                &control_selection,
                &comparison_selection,
                request.bootstrap_replicates,
                &mut rng,
            );
            let redistribution_interval = percentile_interval(
                &control_redistribution,
                &comparison_redistribution,
                request.bootstrap_replicates,
                &mut rng,
            );
            contrasts.push(StateLineageDynamicsContrast {
                state_id: state_id.clone(),
                control: control_summary,
                comparison: comparison_summary,
                population_shift_difference_ppm: population_difference,
                population_shift_call: call_interval(
                    &population_interval,
                    request.minimum_difference_ppm,
                ),
                population_shift_interval: Some(population_interval),
                lineage_selection_difference_ppm: selection_difference,
                lineage_selection_call: call_interval(
                    &selection_interval,
                    request.minimum_difference_ppm,
                ),
                lineage_selection_interval: Some(selection_interval),
                within_lineage_redistribution_difference_ppm: redistribution_difference,
                within_lineage_redistribution_call: call_interval(
                    &redistribution_interval,
                    request.minimum_difference_ppm,
                ),
                within_lineage_redistribution_interval: Some(redistribution_interval),
            });
        } else {
            contrasts.push(StateLineageDynamicsContrast {
                state_id: state_id.clone(),
                control: control_summary,
                comparison: comparison_summary,
                population_shift_difference_ppm: population_difference,
                population_shift_interval: None,
                population_shift_call: ComponentCall::Unresolved,
                lineage_selection_difference_ppm: selection_difference,
                lineage_selection_interval: None,
                lineage_selection_call: ComponentCall::Unresolved,
                within_lineage_redistribution_difference_ppm: redistribution_difference,
                within_lineage_redistribution_interval: None,
                within_lineage_redistribution_call: ComponentCall::Unresolved,
            });
        }
    }
    let mut limitations = vec![
        "arm contrasts are descriptive unless design and confounding controls justify causal interpretation".to_owned(),
        "batch identifiers are preserved in the input digest but this feature does not perform batch adjustment or substitute for blocked randomization".to_owned(),
        "lineage abundance change is not proof of a genetic mechanism or selection cause".to_owned(),
        "within-lineage redistribution is not direct observation of single-cell switching; state-specific growth can also change composition".to_owned(),
    ];
    limitations.sort();
    let mut output = LineageDynamicsAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        control_arm: request.control_arm.clone(),
        comparison_arm: request.comparison_arm.clone(),
        state_order: request.state_order.clone(),
        baseline_timepoint: request.baseline_timepoint,
        followup_timepoint: request.followup_timepoint,
        included_unit_order,
        excluded_unit_order,
        units,
        contrasts,
        minimum_difference_ppm: request.minimum_difference_ppm,
        min_units_per_arm: request.min_units_per_arm,
        min_lineages_per_unit: request.min_lineages_per_unit,
        min_cells_per_unit_timepoint: request.min_cells_per_unit_timepoint,
        bootstrap_replicates: request.bootstrap_replicates,
        bootstrap_seed: request.bootstrap_seed.clone(),
        confidence_level_milli: 950,
        disposition: LineageDynamicsDisposition::Unresolved,
        limitations,
        input_digest,
        digest: ContentHash::of_bytes(b"unsealed-glioma-lineage-dynamics"),
    };
    output.disposition = disposition_from_calls(enough_units, &output.contrasts);
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LineageDynamicsError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> LineageDynamicsRequest {
        LineageDynamicsRequest {
            objective: "decompose glioma state redistribution under perturbation".into(),
            control_arm: "vehicle".into(),
            comparison_arm: "perturbation".into(),
            model_system: GliomaModelSystem::Organoid,
            state_order: vec!["progenitor_like".into(), "mesenchymal_like".into()],
            baseline_timepoint: 0,
            followup_timepoint: 1,
            min_units_per_arm: 3,
            min_lineages_per_unit: 2,
            min_cells_per_unit_timepoint: 50,
            minimum_difference_ppm: 10_000,
            bootstrap_replicates: 999,
            bootstrap_seed: ContentHash::of_bytes(b"lineage-dynamics-test-seed"),
        }
    }

    fn append_unit(
        rows: &mut Vec<LineageStateSnapshot>,
        arm: &str,
        unit: &str,
        baseline: [[u64; 2]; 2],
        followup: [[u64; 2]; 2],
    ) {
        for lineage_index in 0..2 {
            for (timepoint, counts) in [(0, baseline[lineage_index]), (1, followup[lineage_index])]
            {
                let observation_id = format!("{arm}-{unit}-lineage-{lineage_index}-{timepoint}");
                rows.push(LineageStateSnapshot {
                    observation_id: observation_id.clone(),
                    experimental_unit_id: unit.into(),
                    lineage_id: format!("lineage-{lineage_index}"),
                    arm_id: arm.into(),
                    batch_id: "batch-1".into(),
                    model_system: GliomaModelSystem::Organoid,
                    timepoint,
                    state_counts: vec![
                        LineageStateCount {
                            state_id: "progenitor_like".into(),
                            cell_count: counts[0],
                        },
                        LineageStateCount {
                            state_id: "mesenchymal_like".into(),
                            cell_count: counts[1],
                        },
                    ],
                    artifact: LocalArtifactRef {
                        artifact_id: observation_id,
                        content_hash: ContentHash::of_bytes(b"synthetic-preclinical-count-fixture"),
                        content_type: "application/vnd.aurora.glioma-lineage-counts+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                });
            }
        }
    }

    fn balanced_units(
        comparison_baseline: [[u64; 2]; 2],
        comparison_followup: [[u64; 2]; 2],
    ) -> Vec<LineageStateSnapshot> {
        let mut rows = Vec::new();
        for index in 0..3 {
            append_unit(
                &mut rows,
                "vehicle",
                &format!("control-{index}"),
                [[100, 0], [0, 100]],
                [[100, 0], [0, 100]],
            );
            append_unit(
                &mut rows,
                "perturbation",
                &format!("treated-{index}"),
                comparison_baseline,
                comparison_followup,
            );
        }
        rows
    }

    fn high_state(analysis: &LineageDynamicsAnalysis) -> &StateLineageDynamicsContrast {
        &analysis.contrasts[1]
    }

    #[test]
    fn separates_lineage_abundance_shift_from_within_lineage_redistribution() {
        let selection_only = analyze_glioma_lineage_dynamics(
            &request(),
            &balanced_units([[100, 0], [0, 100]], [[100, 0], [0, 300]]),
        )
        .unwrap();
        let selection = high_state(&selection_only);
        assert_eq!(selection.comparison.population_shift_ppm, 250_000);
        assert_eq!(
            selection.comparison.lineage_selection_component_ppm,
            250_000
        );
        assert_eq!(
            selection
                .comparison
                .within_lineage_redistribution_component_ppm,
            0
        );
        assert_eq!(
            selection.lineage_selection_interval,
            Some(BootstrapInterval {
                lower_ppm: 250_000,
                upper_ppm: 250_000
            })
        );

        let redistribution_only = analyze_glioma_lineage_dynamics(
            &request(),
            &balanced_units([[100, 0], [0, 100]], [[0, 100], [0, 100]]),
        )
        .unwrap();
        let redistribution = high_state(&redistribution_only);
        assert_eq!(redistribution.comparison.population_shift_ppm, 500_000);
        assert_eq!(redistribution.comparison.lineage_selection_component_ppm, 0);
        assert_eq!(
            redistribution
                .comparison
                .within_lineage_redistribution_component_ppm,
            500_000
        );

        let mixed = analyze_glioma_lineage_dynamics(
            &request(),
            &balanced_units([[100, 0], [0, 100]], [[80, 20], [0, 300]]),
        )
        .unwrap();
        let contrast = high_state(&mixed);
        assert_eq!(contrast.comparison.population_shift_ppm, 300_000);
        assert_eq!(contrast.comparison.lineage_selection_component_ppm, 225_000);
        assert_eq!(
            contrast
                .comparison
                .within_lineage_redistribution_component_ppm,
            75_000
        );
        assert_eq!(
            contrast.comparison.population_shift_ppm,
            contrast.comparison.lineage_selection_component_ppm
                + contrast
                    .comparison
                    .within_lineage_redistribution_component_ppm
        );
    }

    #[test]
    fn explicit_nondetection_is_data_but_a_missing_timepoint_is_an_error() {
        let mut rows = balanced_units([[100, 0], [0, 100]], [[0, 0], [0, 200]]);
        let result = analyze_glioma_lineage_dynamics(&request(), &rows).unwrap();
        assert_eq!(result.units.len(), 6);
        assert!(result
            .units
            .iter()
            .all(|unit| unit.eligibility == UnitEligibility::Included));

        rows.retain(|row| row.observation_id != "perturbation-treated-0-lineage-0-1");
        assert!(matches!(
            analyze_glioma_lineage_dynamics(&request(), &rows),
            Err(LineageDynamicsError::InvalidInput(_))
        ));
    }

    #[test]
    fn lineages_are_nested_and_bootstrap_counts_independent_units_only() {
        let mut rows = Vec::new();
        for (arm, unit) in [
            ("vehicle", "c0"),
            ("vehicle", "c1"),
            ("perturbation", "t0"),
            ("perturbation", "t1"),
        ] {
            append_unit(
                &mut rows,
                arm,
                unit,
                [[100, 0], [0, 100]],
                [[100, 0], [0, 100]],
            );
        }
        let mut underpowered = request();
        underpowered.min_units_per_arm = 3;
        let result = analyze_glioma_lineage_dynamics(&underpowered, &rows).unwrap();
        assert_eq!(result.units.len(), 4);
        assert_eq!(result.included_unit_order.len(), 4);
        assert_eq!(result.disposition, LineageDynamicsDisposition::Unresolved);
        assert!(result
            .contrasts
            .iter()
            .all(|contrast| contrast.population_shift_interval.is_none()));
    }

    #[test]
    fn canonical_input_order_and_seed_replay_identically() {
        let rows = balanced_units([[100, 0], [0, 100]], [[100, 0], [0, 300]]);
        let forward = analyze_glioma_lineage_dynamics(&request(), &rows).unwrap();
        let mut reversed_rows = rows.clone();
        reversed_rows.reverse();
        let reversed = analyze_glioma_lineage_dynamics(&request(), &reversed_rows).unwrap();
        assert_eq!(forward, reversed);
        forward.validate().unwrap();
    }

    #[test]
    fn rejects_human_data_and_tampered_component_outputs() {
        let mut rows = balanced_units([[100, 0], [0, 100]], [[100, 0], [0, 300]]);
        rows[0].artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_lineage_dynamics(&request(), &rows),
            Err(LineageDynamicsError::InvalidInput(_))
        ));

        let mut analysis = analyze_glioma_lineage_dynamics(
            &request(),
            &balanced_units([[100, 0], [0, 100]], [[100, 0], [0, 300]]),
        )
        .unwrap();
        analysis.units[0].states[0].within_lineage_redistribution_component_ppm += 1;
        analysis.digest = ContentHash::of_value(&digest_input(&analysis)).unwrap();
        assert!(matches!(
            analysis.validate(),
            Err(LineageDynamicsError::InvalidOutput(_))
        ));
    }
}
