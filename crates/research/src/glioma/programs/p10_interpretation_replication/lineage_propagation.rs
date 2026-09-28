//! Fit and validate a lineage-resolved finite-interval glioma cell-state propagation operator.
//!
//! Barcoded state-count trajectories are fit as nonnegative discrete-time operators, with
//! independent experimental units as the resampling unit. The operator describes effective
//! descendant yield and composition over the measured interval; it does not identify individual
//! cell-switch probabilities or continuous-time rates. Implements `GAF-GLIOMA-P10-F02`.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaLineagePropagation1@3";
const SCALE: u64 = 1_000_000;
const EXPERIMENTAL_UNIT_WEIGHT_SCALE: u64 = 1_000_000_000;
const MAX_STATES: usize = 8;
const MAX_SNAPSHOTS: usize = 65_536;
const MAX_COUNT: u32 = 1_000_000_000;
const MAX_CORRECTED_COUNT: u64 = 1_000_000_000_000;
const MIN_BOOTSTRAP: usize = 99;
const MAX_BOOTSTRAP: usize = 499;
const MAX_SWEEPS: usize = 250;
const MAX_OPERATOR_PPM: u64 = 50_000_000;
const MAX_BOOTSTRAP_WORK: u128 = 50_000_000;
pub const MAX_LINEAGE_PROPAGATION_BATCH_SENSITIVITY_BATCHES: usize = 128;
const MAX_BATCH_SENSITIVITY_WORK: u128 = 50_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_arm: String,
    pub treatment_arm: String,
    /// State order is fixed for every snapshot and every output operator.
    pub state_order: Vec<String>,
    pub min_units_per_arm: usize,
    pub min_lineages_per_arm: usize,
    pub ridge_penalty_ppm: u32,
    pub max_coefficient_ppm: u64,
    pub max_prediction_error_ppm: u32,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: ContentHash,
    pub confidence_level_milli: u16,
    /// Smallest treatment-control coefficient change considered practically material.
    pub minimum_effect_ppm: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationSnapshot {
    pub observation_id: String,
    /// Independent culture, animal, organoid batch, or other preclinical experimental unit.
    pub experimental_unit_id: String,
    /// Heritable barcode or lineage identifier nested within the experimental unit.
    pub lineage_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    /// Technical assay/processing batch; distinct from the biological experimental unit.
    pub assay_batch_id: String,
    pub timepoint_day: u32,
    /// Counts in the request's declared state order. A missing row is not a zero-count row.
    pub state_counts: Vec<u32>,
    /// Effective fraction of the source population represented in this snapshot, in ppm.
    pub capture_fraction_ppm: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PpmInterval {
    pub lower: i64,
    pub upper: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoefficientDisposition {
    Estimable,
    Boundary,
    RankUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationCoefficient {
    pub from_state: String,
    pub to_state: String,
    /// Expected destination-state descendants per source-state cell over one observation interval.
    pub coefficient_ppm: u64,
    pub bootstrap_interval_ppm: PpmInterval,
    pub disposition: CoefficientDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropagationDestinationShare {
    pub to_state: String,
    pub share_ppm: u32,
    pub bootstrap_interval_ppm: PpmInterval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStatePropagation {
    pub from_state: String,
    /// Column sum of the propagation operator: effective descendants per source-state cell.
    pub effective_descendant_yield_ppm: u64,
    pub yield_interval_ppm: PpmInterval,
    /// Column-normalized destination composition. This is not a per-cell transition probability.
    pub destination_composition: Vec<PropagationDestinationShare>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationOperator {
    pub arm_id: String,
    pub coefficients: Vec<LineagePropagationCoefficient>,
    pub source_state_summaries: Vec<SourceStatePropagation>,
    pub biological_replicate_count: u32,
    pub complete_lineage_count: u32,
    pub training_pair_count: u32,
    /// Numerical rank estimate of the source-state design matrix (0..=number of states).
    pub source_design_rank: u16,
    pub boundary_coefficient_count: u32,
    pub converged: bool,
}

/// One paired draw from the arm-specific, independent-unit bootstrap distribution. Coefficient
/// vectors are row-major destination × source matrices in `LineagePropagationAnalysis.state_order`.
/// The pairing is a deterministic Monte Carlo coupling for acquisition design, not biological
/// pairing of control and treatment experimental units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationBootstrapDraw {
    pub replicate_index: u16,
    pub control_coefficients_ppm: Vec<u64>,
    pub treatment_coefficients_ppm: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationContrast {
    pub from_state: String,
    pub to_state: String,
    pub treatment_minus_control_ppm: i64,
    pub bootstrap_interval_ppm: PpmInterval,
    pub disposition: EffectDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineagePropagationBatchSensitivityDisposition {
    Stable,
    Fragile,
    Partial,
    Unresolved,
}

/// Exact leave-one-assay-batch-out refit summary for one source-state column of the propagation
/// operator. This is a robustness diagnostic, not a replacement estimate or causal test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationBatchSensitivity {
    pub from_state: String,
    pub batch_count: u32,
    pub evaluated_batch_count: u32,
    pub maximum_leave_one_batch_out_deviation_ppm: u64,
    /// Unevaluable omission folds / all folds on a 0..=1,000 scale.
    pub robustness_gap_milli: u16,
    pub direction_reversal_count: u32,
    pub unstable_contrast_order: Vec<String>,
    pub disposition: LineagePropagationBatchSensitivityDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectDisposition {
    BeyondPracticalMargin,
    WithinPracticalMargin,
    Inconclusive,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldOutForecast {
    pub arm_id: String,
    pub held_out_interval_days: u32,
    pub experimental_unit_count: u32,
    pub lineage_count: u32,
    /// Equal-weight mean, by experimental unit, of total-variation distance between observed and
    /// predicted destination-state compositions, in ppm. Extinct lineages are scored explicitly.
    pub mean_composition_error_ppm: u32,
    /// Equal-weight mean, by experimental unit, of relative total-abundance prediction error, ppm.
    pub mean_abundance_error_ppm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineagePropagationDisposition {
    Qualified,
    PartiallyIdentified,
    PredictionFailure,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub state_order: Vec<String>,
    pub control_arm: String,
    pub treatment_arm: String,
    pub observation_timepoints_day: Vec<u32>,
    pub interval_days: u32,
    pub control: LineagePropagationOperator,
    pub treatment: LineagePropagationOperator,
    pub contrasts: Vec<LineagePropagationContrast>,
    pub batch_sensitivity: Vec<LineagePropagationBatchSensitivity>,
    pub held_out_forecasts: Vec<HeldOutForecast>,
    pub incomplete_lineage_count: u32,
    pub zero_source_interval_count: u32,
    pub ridge_penalty_ppm: u32,
    pub max_coefficient_ppm: u64,
    pub max_prediction_error_ppm: u32,
    pub bootstrap_replicates: usize,
    /// Bounded joint particle approximation exported for candidate-specific P06 acquisition design.
    pub bootstrap_draws: Vec<LineagePropagationBootstrapDraw>,
    pub bootstrap_seed: ContentHash,
    pub confidence_level_milli: u16,
    pub minimum_effect_ppm: u64,
    pub disposition: LineagePropagationDisposition,
    pub limitations: Vec<String>,
    pub input_digest: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineagePropagationError {
    #[error("lineage propagation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage propagation input is invalid: {0}")]
    InvalidInput(String),
    #[error("lineage propagation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage propagation digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct Trajectory {
    arm: String,
    unit: String,
    lineage: String,
    counts: Vec<Vec<i64>>,
    assay_batch_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct FitRow {
    unit: String,
    lineage: String,
    source_batch_id: String,
    destination_batch_id: String,
    x: Vec<i64>,
    y: Vec<i64>,
}

#[derive(Debug, Clone)]
struct Fit {
    matrix: Vec<u64>,
    rank: usize,
    converged: bool,
}

fn validate_request(request: &LineagePropagationRequest) -> Result<(), LineagePropagationError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || !(2..=MAX_STATES).contains(&request.state_order.len())
        || request
            .state_order
            .iter()
            .any(|state| state.trim().is_empty())
        || request.state_order.iter().collect::<BTreeSet<_>>().len() != request.state_order.len()
        || request.min_units_per_arm < 2
        || request.min_lineages_per_arm < request.state_order.len()
        || request.ridge_penalty_ppm > 1_000_000
        || request.max_coefficient_ppm == 0
        || request.max_coefficient_ppm > MAX_OPERATOR_PPM
        || request.max_prediction_error_ppm > 1_000_000
        || !(MIN_BOOTSTRAP..=MAX_BOOTSTRAP).contains(&request.bootstrap_replicates)
        || !(800..=999).contains(&request.confidence_level_milli)
        || request.minimum_effect_ppm == 0
        || request.minimum_effect_ppm > request.max_coefficient_ppm
    {
        return Err(LineagePropagationError::InvalidRequest(
            "objective, two arms, 2..=8 unique states, replicate/lineage floors, regularization, coefficient/prediction bounds, bootstrap, confidence, or effect margin is invalid".into(),
        ));
    }
    Ok(())
}

fn corrected_count(count: u32, capture_ppm: u32) -> Result<u64, LineagePropagationError> {
    let numerator = u128::from(count) * u128::from(SCALE);
    let corrected = (numerator + u128::from(capture_ppm) / 2) / u128::from(capture_ppm);
    if corrected > u128::from(MAX_CORRECTED_COUNT) {
        return Err(LineagePropagationError::InvalidInput(
            "capture-corrected cell count exceeds the bounded numeric range".into(),
        ));
    }
    Ok(corrected as u64)
}

fn validate_and_prepare(
    request: &LineagePropagationRequest,
    snapshots: &[LineagePropagationSnapshot],
) -> Result<(Vec<Trajectory>, Vec<u32>, u32), LineagePropagationError> {
    if snapshots.is_empty() || snapshots.len() > MAX_SNAPSHOTS {
        return Err(LineagePropagationError::InvalidInput(
            "snapshot set is empty or exceeds bounded capacity".into(),
        ));
    }
    let allowed_arms =
        BTreeSet::from([request.control_arm.as_str(), request.treatment_arm.as_str()]);
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut unit_arm = BTreeMap::<String, String>::new();
    let mut timepoints = BTreeSet::new();
    let mut corrected = BTreeMap::<(String, String, String, u32), (Vec<u64>, String)>::new();
    let mut max_cell_count = 0_u64;
    for snapshot in snapshots {
        if snapshot.observation_id.trim().is_empty()
            || snapshot.experimental_unit_id.trim().is_empty()
            || snapshot.lineage_id.trim().is_empty()
            || snapshot.assay_batch_id.trim().is_empty()
            || !allowed_arms.contains(snapshot.arm_id.as_str())
            || snapshot.model_system != request.model_system
            || snapshot.state_counts.len() != request.state_order.len()
            || snapshot.state_counts.iter().any(|count| *count > MAX_COUNT)
            || !(1_000..=1_000_000).contains(&snapshot.capture_fraction_ppm)
            || !ids.insert(snapshot.observation_id.clone())
            || !keys.insert((
                snapshot.arm_id.clone(),
                snapshot.experimental_unit_id.clone(),
                snapshot.lineage_id.clone(),
                snapshot.timepoint_day,
            ))
        {
            return Err(LineagePropagationError::InvalidInput(
                "snapshot identity, arm/model, ordered state vector, count/capture bound, or uniqueness is invalid".into(),
            ));
        }
        if unit_arm
            .insert(
                snapshot.experimental_unit_id.clone(),
                snapshot.arm_id.clone(),
            )
            .is_some_and(|arm| arm != snapshot.arm_id)
        {
            return Err(LineagePropagationError::InvalidInput(
                "an experimental unit appears in more than one arm".into(),
            ));
        }
        snapshot
            .artifact
            .validate()
            .map_err(|error| LineagePropagationError::InvalidInput(error.to_string()))?;
        if !snapshot.artifact.local_only
            || snapshot.artifact.contains_human_data
            || snapshot.artifact.contains_direct_identifiers
        {
            return Err(LineagePropagationError::InvalidInput(
                "only local preclinical artifacts without human data or direct identifiers are accepted".into(),
            ));
        }
        let state_counts = snapshot
            .state_counts
            .iter()
            .map(|count| corrected_count(*count, snapshot.capture_fraction_ppm))
            .collect::<Result<Vec<_>, _>>()?;
        max_cell_count = max_cell_count.max(state_counts.iter().copied().max().unwrap_or(0));
        timepoints.insert(snapshot.timepoint_day);
        corrected.insert(
            (
                snapshot.arm_id.clone(),
                snapshot.experimental_unit_id.clone(),
                snapshot.lineage_id.clone(),
                snapshot.timepoint_day,
            ),
            (state_counts, snapshot.assay_batch_id.clone()),
        );
    }
    if max_cell_count == 0 {
        return Err(LineagePropagationError::InvalidInput(
            "all corrected state counts are zero".into(),
        ));
    }
    let timepoints = timepoints.into_iter().collect::<Vec<_>>();
    if timepoints.len() < 3 {
        return Err(LineagePropagationError::InvalidInput(
            "at least three scheduled timepoints are required for temporal holdout validation"
                .into(),
        ));
    }
    let interval = timepoints[1] - timepoints[0];
    if interval == 0
        || timepoints
            .windows(2)
            .any(|pair| pair[1] - pair[0] != interval)
    {
        return Err(LineagePropagationError::InvalidInput(
            "timepoints must use one fixed, positive observation interval".into(),
        ));
    }
    let normalize = |counts: &[u64]| -> Result<Vec<i64>, LineagePropagationError> {
        counts
            .iter()
            .map(|count| {
                let scaled =
                    (u128::from(*count) * u128::from(SCALE) + u128::from(max_cell_count) / 2)
                        / u128::from(max_cell_count);
                if *count > 0 && scaled == 0 {
                    return Err(LineagePropagationError::InvalidInput(
                        "count dynamic range exceeds fixed-point precision; provide a narrower, pre-normalized analysis slice".into(),
                    ));
                }
                Ok(scaled as i64)
            })
            .collect()
    };
    let mut grouped =
        BTreeMap::<(String, String, String), BTreeMap<u32, (Vec<i64>, String)>>::new();
    for ((arm, unit, lineage, timepoint), (counts, assay_batch_id)) in corrected {
        grouped
            .entry((arm, unit, lineage))
            .or_default()
            .insert(timepoint, (normalize(&counts)?, assay_batch_id));
    }
    let mut complete = Vec::new();
    let mut incomplete = 0_u32;
    for ((arm, unit, lineage), series) in grouped {
        if timepoints.iter().all(|time| series.contains_key(time)) {
            let complete_series = timepoints
                .iter()
                .map(|time| series.get(time).expect("complete series checked"))
                .collect::<Vec<_>>();
            complete.push(Trajectory {
                arm,
                unit,
                lineage,
                counts: complete_series
                    .iter()
                    .map(|(counts, _)| counts.clone())
                    .collect(),
                assay_batch_ids: complete_series
                    .iter()
                    .map(|(_, batch_id)| batch_id.clone())
                    .collect(),
            });
        } else {
            incomplete = incomplete.saturating_add(1);
        }
    }
    for arm in [&request.control_arm, &request.treatment_arm] {
        let rows = complete
            .iter()
            .filter(|row| &row.arm == arm)
            .collect::<Vec<_>>();
        let units = rows
            .iter()
            .map(|row| row.unit.as_str())
            .collect::<BTreeSet<_>>();
        if units.len() < request.min_units_per_arm || rows.len() < request.min_lineages_per_arm {
            return Err(LineagePropagationError::InvalidInput(format!(
                "arm {arm} has {} complete lineages across {} independent units; declared floors are {} and {}",
                rows.len(),
                units.len(),
                request.min_lineages_per_arm,
                request.min_units_per_arm
            )));
        }
    }
    complete.sort_by(|left, right| {
        (&left.arm, &left.unit, &left.lineage).cmp(&(&right.arm, &right.unit, &right.lineage))
    });
    Ok((complete, timepoints, incomplete))
}

fn make_rows(trajectories: &[Trajectory], train: bool) -> (Vec<FitRow>, u32) {
    let mut rows = Vec::new();
    let mut zero = 0_u32;
    for trajectory in trajectories {
        let indices = if train {
            0..trajectory.counts.len() - 2
        } else {
            trajectory.counts.len() - 2..trajectory.counts.len() - 1
        };
        for index in indices {
            let x = &trajectory.counts[index];
            let y = &trajectory.counts[index + 1];
            if x.iter().all(|value| *value == 0) {
                zero = zero.saturating_add(1);
                continue;
            }
            rows.push(FitRow {
                unit: trajectory.unit.clone(),
                lineage: trajectory.lineage.clone(),
                source_batch_id: trajectory.assay_batch_ids[index].clone(),
                destination_batch_id: trajectory.assay_batch_ids[index + 1].clone(),
                x: x.clone(),
                y: y.clone(),
            });
        }
    }
    (rows, zero)
}

fn design_rank(gram: &[Vec<i128>]) -> usize {
    let n = gram.len();
    let mut matrix = vec![vec![0.0_f64; n]; n];
    for row in 0..n {
        for col in 0..n {
            let denominator = (gram[row][row].max(0) as f64 * gram[col][col].max(0) as f64).sqrt();
            matrix[row][col] = if denominator == 0.0 {
                0.0
            } else {
                gram[row][col] as f64 / denominator
            };
        }
    }
    let mut rank = 0;
    for column in 0..n {
        let pivot = (rank..n).max_by(|left, right| {
            matrix[*left][column]
                .abs()
                .total_cmp(&matrix[*right][column].abs())
        });
        let Some(pivot) = pivot else { break };
        if matrix[pivot][column].abs() < 1e-8 {
            continue;
        }
        matrix.swap(rank, pivot);
        let divisor = matrix[rank][column];
        for col in column..n {
            matrix[rank][col] /= divisor;
        }
        for row in 0..n {
            if row != rank {
                let multiplier = matrix[row][column];
                for col in column..n {
                    matrix[row][col] -= multiplier * matrix[rank][col];
                }
            }
        }
        rank += 1;
        if rank == n {
            break;
        }
    }
    rank
}

fn fit_operator(
    rows: &[FitRow],
    weights: &BTreeMap<String, u64>,
    states: usize,
    ridge_ppm: u32,
    max_coefficient_ppm: u64,
) -> Fit {
    let mut gram = vec![vec![0_i128; states]; states];
    let mut cross = vec![vec![0_i128; states]; states];
    for row in rows {
        let Some(&weight) = weights.get(&row.unit) else {
            continue;
        };
        if weight == 0 {
            continue;
        }
        let weight = i128::from(weight);
        for from in 0..states {
            for other in 0..states {
                gram[from][other] += i128::from(row.x[from]) * i128::from(row.x[other]) * weight;
            }
            for to in 0..states {
                cross[to][from] += i128::from(row.x[from]) * i128::from(row.y[to]) * weight;
            }
        }
    }
    let rank = design_rank(&gram);
    let trace = (0..states).map(|i| gram[i][i]).sum::<i128>();
    let ridge = if ridge_ppm == 0 {
        0
    } else {
        ((trace * i128::from(ridge_ppm)) / (states as i128 * i128::from(SCALE))).max(1)
    };
    let mut matrix = vec![0_u64; states * states];
    let mut all_converged = true;
    for to in 0..states {
        let mut beta = vec![0_i128; states];
        let mut converged = false;
        for _ in 0..MAX_SWEEPS {
            let mut max_change = 0_i128;
            for from in 0..states {
                let diagonal = gram[from][from] + ridge;
                if diagonal == 0 {
                    continue;
                }
                let residual = cross[to][from] * i128::from(SCALE)
                    - (0..states)
                        .map(|other| gram[from][other] * beta[other])
                        .sum::<i128>();
                let updated =
                    (beta[from] + residual / diagonal).clamp(0, i128::from(max_coefficient_ppm));
                max_change = max_change.max((updated - beta[from]).abs());
                beta[from] = updated;
            }
            if max_change <= 1 {
                converged = true;
                break;
            }
        }
        all_converged &= converged;
        for from in 0..states {
            matrix[to * states + from] = beta[from] as u64;
        }
    }
    Fit {
        matrix,
        rank,
        converged: all_converged,
    }
}

fn practical_direction(effect_ppm: i64, margin_ppm: u64) -> i8 {
    if effect_ppm.unsigned_abs() < margin_ppm {
        0
    } else if effect_ppm > 0 {
        1
    } else {
        -1
    }
}

fn represented_units_and_lineages(rows: &[FitRow]) -> (usize, usize) {
    (
        rows.iter()
            .map(|row| row.unit.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        rows.iter()
            .map(|row| row.lineage.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
    )
}

fn unit_weights_for_rows(rows: &[FitRow]) -> BTreeMap<String, u64> {
    let units = rows
        .iter()
        .map(|row| row.unit.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unit_weights(rows, &units)
}

fn batch_sensitivity(
    request: &LineagePropagationRequest,
    states: &[String],
    training_rows_by_arm: &BTreeMap<String, Vec<FitRow>>,
    full_by_arm: &BTreeMap<String, Fit>,
) -> Vec<LineagePropagationBatchSensitivity> {
    let batch_ids = training_rows_by_arm
        .values()
        .flatten()
        .flat_map(|row| [&row.source_batch_id, &row.destination_batch_id])
        .cloned()
        .collect::<BTreeSet<_>>();
    let batch_count = batch_ids.len().min(u32::MAX as usize) as u32;
    let full_fits_estimable = [&request.control_arm, &request.treatment_arm]
        .iter()
        .all(|arm| {
            full_by_arm
                .get(*arm)
                .is_some_and(|fit| fit.rank == states.len() && fit.converged)
        });
    let total_rows = training_rows_by_arm.values().map(Vec::len).sum::<usize>();
    let work = (batch_ids.len() as u128)
        .saturating_mul(2)
        .saturating_mul(states.len() as u128)
        .saturating_mul(states.len() as u128)
        .saturating_mul(total_rows.saturating_add(MAX_SWEEPS) as u128);
    let early_unresolved = if !full_fits_estimable {
        Some("the primary arm operators are rank deficient or unconverged".to_string())
    } else if batch_ids.len() < 2 {
        Some("fewer than two assay batches contribute to the training intervals".to_string())
    } else if batch_ids.len() > MAX_LINEAGE_PROPAGATION_BATCH_SENSITIVITY_BATCHES {
        Some(format!(
            "batch count exceeds the bounded leave-one-batch-out limit of {MAX_LINEAGE_PROPAGATION_BATCH_SENSITIVITY_BATCHES}"
        ))
    } else if work > MAX_BATCH_SENSITIVITY_WORK {
        Some("leave-one-batch-out refit work exceeds the bounded sensitivity budget".to_string())
    } else {
        None
    };
    if let Some(rationale) = early_unresolved {
        return states
            .iter()
            .map(|from_state| LineagePropagationBatchSensitivity {
                from_state: from_state.clone(),
                batch_count,
                evaluated_batch_count: 0,
                maximum_leave_one_batch_out_deviation_ppm: 0,
                robustness_gap_milli: 1_000,
                direction_reversal_count: 0,
                unstable_contrast_order: Vec::new(),
                disposition: LineagePropagationBatchSensitivityDisposition::Unresolved,
                rationale: rationale.clone(),
            })
            .collect();
    }

    let mut output = Vec::with_capacity(states.len());
    for (source_index, from_state) in states.iter().enumerate() {
        let mut evaluated_batch_count = 0_u32;
        let mut maximum_deviation = 0_u64;
        let mut direction_reversal_count = 0_u32;
        let mut unstable_contrasts = BTreeSet::new();
        for omitted_batch in &batch_ids {
            let mut fits = BTreeMap::new();
            let mut fold_estimable = true;
            for arm in [&request.control_arm, &request.treatment_arm] {
                let rows = training_rows_by_arm[arm]
                    .iter()
                    .filter(|row| {
                        row.source_batch_id != *omitted_batch
                            && row.destination_batch_id != *omitted_batch
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let (unit_count, lineage_count) = represented_units_and_lineages(&rows);
                if unit_count < request.min_units_per_arm
                    || lineage_count < request.min_lineages_per_arm
                    || rows.is_empty()
                {
                    fold_estimable = false;
                    break;
                }
                let weights = unit_weights_for_rows(&rows);
                let fit = fit_operator(
                    &rows,
                    &weights,
                    states.len(),
                    request.ridge_penalty_ppm,
                    request.max_coefficient_ppm,
                );
                if fit.rank != states.len() || !fit.converged {
                    fold_estimable = false;
                    break;
                }
                fits.insert(arm.clone(), fit);
            }
            if !fold_estimable {
                continue;
            }
            evaluated_batch_count = evaluated_batch_count.saturating_add(1);
            let control = &fits[&request.control_arm];
            let treatment = &fits[&request.treatment_arm];
            let full_control = &full_by_arm[&request.control_arm];
            let full_treatment = &full_by_arm[&request.treatment_arm];
            for destination_index in 0..states.len() {
                let coefficient_index = destination_index * states.len() + source_index;
                let full_effect = full_treatment.matrix[coefficient_index] as i64
                    - full_control.matrix[coefficient_index] as i64;
                let leave_one_out_effect = treatment.matrix[coefficient_index] as i64
                    - control.matrix[coefficient_index] as i64;
                let deviation = leave_one_out_effect.abs_diff(full_effect);
                maximum_deviation = maximum_deviation.max(deviation);
                let full_direction = practical_direction(full_effect, request.minimum_effect_ppm);
                let omitted_direction =
                    practical_direction(leave_one_out_effect, request.minimum_effect_ppm);
                if full_direction != 0
                    && omitted_direction != 0
                    && full_direction != omitted_direction
                {
                    direction_reversal_count = direction_reversal_count.saturating_add(1);
                }
                if full_direction != omitted_direction || deviation >= request.minimum_effect_ppm {
                    unstable_contrasts
                        .insert(format!("{}:{}", from_state, states[destination_index]));
                }
            }
        }
        let robustness_gap_milli = if batch_count == 0 {
            1_000
        } else {
            ((u64::from(batch_count.saturating_sub(evaluated_batch_count)) * 1_000)
                / u64::from(batch_count)) as u16
        };
        let disposition = if evaluated_batch_count == 0 {
            LineagePropagationBatchSensitivityDisposition::Unresolved
        } else if direction_reversal_count > 0 || !unstable_contrasts.is_empty() {
            LineagePropagationBatchSensitivityDisposition::Fragile
        } else if evaluated_batch_count < batch_count {
            LineagePropagationBatchSensitivityDisposition::Partial
        } else {
            LineagePropagationBatchSensitivityDisposition::Stable
        };
        let rationale = match disposition {
            LineagePropagationBatchSensitivityDisposition::Stable => {
                "all assay-batch omissions retained the primary lineage-operator effects within the practical margin".into()
            }
            LineagePropagationBatchSensitivityDisposition::Fragile => {
                "one or more assay-batch omissions materially changed or reversed a lineage-operator effect".into()
            }
            LineagePropagationBatchSensitivityDisposition::Partial => {
                "some assay-batch omissions fell below the independent-unit, lineage, rank, or convergence floor".into()
            }
            LineagePropagationBatchSensitivityDisposition::Unresolved => {
                "no assay-batch omission retained sufficient independent-unit and full-rank lineage support".into()
            }
        };
        output.push(LineagePropagationBatchSensitivity {
            from_state: from_state.clone(),
            batch_count,
            evaluated_batch_count,
            maximum_leave_one_batch_out_deviation_ppm: maximum_deviation,
            robustness_gap_milli,
            direction_reversal_count,
            unstable_contrast_order: unstable_contrasts.into_iter().collect(),
            disposition,
            rationale,
        });
    }
    output
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

fn quantile_interval(mut values: Vec<i64>, confidence_milli: u16) -> PpmInterval {
    values.sort_unstable();
    let last = values.len().saturating_sub(1);
    let tail = u64::from(1_000 - confidence_milli) / 2;
    let lower = (last as u64 * tail / 1_000) as usize;
    let upper = (last as u64 * (1_000 - tail)).div_ceil(1_000).min(last as u64) as usize;
    PpmInterval {
        lower: values[lower],
        upper: values[upper],
    }
}

/// Keeps row-rich experimental units from gaining extra influence just by carrying more barcodes.
fn unit_weights(rows: &[FitRow], units: &[String]) -> BTreeMap<String, u64> {
    let mut row_counts = BTreeMap::<String, u64>::new();
    for row in rows {
        *row_counts.entry(row.unit.clone()).or_default() += 1;
    }
    let mut multiplicities = BTreeMap::<String, u64>::new();
    for unit in units {
        *multiplicities.entry(unit.clone()).or_default() += 1;
    }
    multiplicities
        .into_iter()
        .filter_map(|(unit, multiplicity)| {
            let row_count = *row_counts.get(&unit)?;
            let per_row_weight = (EXPERIMENTAL_UNIT_WEIGHT_SCALE / row_count).max(1);
            Some((unit, per_row_weight.saturating_mul(multiplicity)))
        })
        .collect()
}

fn arm_rows(trajectories: &[Trajectory], arm: &str, train: bool) -> Vec<FitRow> {
    make_rows(
        &trajectories
            .iter()
            .filter(|trajectory| trajectory.arm == arm)
            .cloned()
            .collect::<Vec<_>>(),
        train,
    )
    .0
}

fn bootstrap_fits(
    rows: &[FitRow],
    request: &LineagePropagationRequest,
    rng: &mut SplitMix64,
) -> (Vec<Fit>, Fit) {
    let mut units = rows
        .iter()
        .map(|row| row.unit.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    units.sort();
    let full_weights = unit_weights(rows, &units);
    let full = fit_operator(
        rows,
        &full_weights,
        request.state_order.len(),
        request.ridge_penalty_ppm,
        request.max_coefficient_ppm,
    );
    let mut samples = Vec::with_capacity(request.bootstrap_replicates);
    for _ in 0..request.bootstrap_replicates {
        let draw = (0..units.len())
            .map(|_| units[rng.index(units.len())].clone())
            .collect::<Vec<_>>();
        samples.push(fit_operator(
            rows,
            &unit_weights(rows, &draw),
            request.state_order.len(),
            request.ridge_penalty_ppm,
            request.max_coefficient_ppm,
        ));
    }
    (samples, full)
}

fn normalize_to_shares(values: &[u128]) -> Vec<u32> {
    let total = values.iter().sum::<u128>();
    if total == 0 {
        return vec![0; values.len()];
    }
    let mut shares = values
        .iter()
        .map(|value| (value * u128::from(SCALE) / total) as u32)
        .collect::<Vec<_>>();
    let assigned = shares.iter().map(|value| *value as u64).sum::<u64>();
    if let Some((index, _)) = values.iter().enumerate().max_by_key(|(_, value)| *value) {
        shares[index] = shares[index].saturating_add((SCALE - assigned) as u32);
    }
    shares
}

fn build_operator(
    arm: &str,
    states: &[String],
    samples: &[Fit],
    full: &Fit,
    units: usize,
    lineages: usize,
    training_pairs: usize,
    confidence: u16,
    max_coefficient_ppm: u64,
) -> LineagePropagationOperator {
    let count = states.len();
    let boundary_count = (0..count * count)
        .filter(|index| {
            full.matrix[*index] == max_coefficient_ppm
                || samples
                    .iter()
                    .any(|sample| sample.matrix[*index] == max_coefficient_ppm)
        })
        .count();
    let mut coefficients = Vec::with_capacity(count * count);
    for to in 0..count {
        for from in 0..count {
            let index = to * count + from;
            let values = samples
                .iter()
                .map(|sample| sample.matrix[index] as i64)
                .collect::<Vec<_>>();
            let estimate = full.matrix[index];
            coefficients.push(LineagePropagationCoefficient {
                from_state: states[from].clone(),
                to_state: states[to].clone(),
                coefficient_ppm: estimate,
                bootstrap_interval_ppm: quantile_interval(values, confidence),
                disposition: if full.rank < count {
                    CoefficientDisposition::RankUnresolved
                } else if estimate == max_coefficient_ppm
                    || samples
                        .iter()
                        .any(|sample| sample.matrix[index] == max_coefficient_ppm)
                {
                    CoefficientDisposition::Boundary
                } else {
                    CoefficientDisposition::Estimable
                },
            });
        }
    }
    let mut source_state_summaries = Vec::with_capacity(count);
    for from in 0..count {
        let estimate_yield = (0..count)
            .map(|to| full.matrix[to * count + from])
            .sum::<u64>();
        let bootstrap_yields = samples
            .iter()
            .map(|sample| {
                (0..count)
                    .map(|to| sample.matrix[to * count + from])
                    .sum::<u64>() as i64
            })
            .collect::<Vec<_>>();
        let mut destinations = Vec::with_capacity(count);
        for to in 0..count {
            let share_samples = samples
                .iter()
                .map(|sample| {
                    let denominator = (0..count)
                        .map(|dest| sample.matrix[dest * count + from])
                        .sum::<u64>();
                    if denominator == 0 {
                        0
                    } else {
                        (u128::from(sample.matrix[to * count + from]) * u128::from(SCALE)
                            / u128::from(denominator)) as i64
                    }
                })
                .collect::<Vec<_>>();
            let full_column = (0..count)
                .map(|dest| u128::from(full.matrix[dest * count + from]))
                .collect::<Vec<_>>();
            let share = if estimate_yield == 0 {
                0
            } else {
                normalize_to_shares(&full_column)[to]
            };
            destinations.push(PropagationDestinationShare {
                to_state: states[to].clone(),
                share_ppm: share,
                bootstrap_interval_ppm: quantile_interval(share_samples, confidence),
            });
        }
        source_state_summaries.push(SourceStatePropagation {
            from_state: states[from].clone(),
            effective_descendant_yield_ppm: estimate_yield,
            yield_interval_ppm: quantile_interval(bootstrap_yields, confidence),
            destination_composition: destinations,
        });
    }
    LineagePropagationOperator {
        arm_id: arm.into(),
        coefficients,
        source_state_summaries,
        biological_replicate_count: units as u32,
        complete_lineage_count: lineages as u32,
        training_pair_count: training_pairs as u32,
        source_design_rank: full.rank as u16,
        boundary_coefficient_count: boundary_count as u32,
        converged: full.converged && samples.iter().all(|sample| sample.converged),
    }
}

fn predict(operator: &[u64], source: &[i64], states: usize) -> Vec<u128> {
    (0..states)
        .map(|to| {
            (0..states)
                .map(|from| u128::from(operator[to * states + from]) * source[from] as u128)
                .sum::<u128>()
                / u128::from(SCALE)
        })
        .collect()
}

fn forecast(
    trajectories: &[Trajectory],
    arm: &str,
    operator: &[u64],
    interval_days: u32,
    states: usize,
) -> HeldOutForecast {
    let mut unit_composition = BTreeMap::<String, Vec<u32>>::new();
    let mut unit_abundance = BTreeMap::<String, Vec<u32>>::new();
    let mut lineages = 0_u32;
    for trajectory in trajectories
        .iter()
        .filter(|trajectory| trajectory.arm == arm)
    {
        let observed = &trajectory.counts[trajectory.counts.len() - 1];
        // The held-out forecast uses the penultimate point; fit rows exclude this interval.
        let source = &trajectory.counts[trajectory.counts.len() - 2];
        let predicted = predict(operator, source, states);
        let observed = observed
            .iter()
            .map(|value| *value as u128)
            .collect::<Vec<_>>();
        let predicted_total = predicted.iter().sum::<u128>();
        let observed_total = observed.iter().sum::<u128>();
        let predicted_shares = normalize_to_shares(&predicted);
        let observed_shares = normalize_to_shares(&observed);
        let tv = if predicted_total == 0 && observed_total == 0 {
            0
        } else if predicted_total == 0 || observed_total == 0 {
            1_000_000
        } else {
            (predicted_shares
                .iter()
                .zip(&observed_shares)
                .map(|(left, right)| left.abs_diff(*right) as u64)
                .sum::<u64>()
                / 2) as u32
        };
        let abundance_error = if observed_total == 0 {
            if predicted_total == 0 {
                0
            } else {
                1_000_000
            }
        } else {
            (predicted_total.abs_diff(observed_total) * u128::from(SCALE) / observed_total)
                .min(u128::from(SCALE)) as u32
        };
        unit_composition
            .entry(trajectory.unit.clone())
            .or_default()
            .push(tv);
        unit_abundance
            .entry(trajectory.unit.clone())
            .or_default()
            .push(abundance_error);
        lineages = lineages.saturating_add(1);
    }
    let mean_by_unit = |values: &BTreeMap<String, Vec<u32>>| -> u32 {
        if values.is_empty() {
            return 1_000_000;
        }
        let per_unit = values
            .values()
            .map(|unit| unit.iter().map(|value| u64::from(*value)).sum::<u64>() / unit.len() as u64)
            .collect::<Vec<_>>();
        (per_unit.iter().sum::<u64>() / per_unit.len() as u64) as u32
    };
    HeldOutForecast {
        arm_id: arm.into(),
        held_out_interval_days: interval_days,
        experimental_unit_count: unit_composition.len() as u32,
        lineage_count: lineages,
        mean_composition_error_ppm: mean_by_unit(&unit_composition),
        mean_abundance_error_ppm: mean_by_unit(&unit_abundance),
    }
}

fn digest_input(output: &LineagePropagationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "state_order": output.state_order,
        "arms": [output.control_arm, output.treatment_arm],
        "observation_timepoints_day": output.observation_timepoints_day,
        "interval_days": output.interval_days,
        "control": output.control,
        "treatment": output.treatment,
        "contrasts": output.contrasts,
        "batch_sensitivity": output.batch_sensitivity,
        "held_out_forecasts": output.held_out_forecasts,
        "incomplete_lineage_count": output.incomplete_lineage_count,
        "zero_source_interval_count": output.zero_source_interval_count,
        "ridge_penalty_ppm": output.ridge_penalty_ppm,
        "max_coefficient_ppm": output.max_coefficient_ppm,
        "max_prediction_error_ppm": output.max_prediction_error_ppm,
        "bootstrap_replicates": output.bootstrap_replicates,
        "bootstrap_draws": output.bootstrap_draws,
        "bootstrap_seed": output.bootstrap_seed,
        "confidence_level_milli": output.confidence_level_milli,
        "minimum_effect_ppm": output.minimum_effect_ppm,
        "disposition": output.disposition,
        "limitations": output.limitations,
        "input_digest": output.input_digest,
    })
}

impl LineagePropagationAnalysis {
    pub fn validate(&self) -> Result<(), LineagePropagationError> {
        let states = self.state_order.len();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !(2..=MAX_STATES).contains(&states)
            || self.interval_days == 0
            || self.observation_timepoints_day.len() < 3
            || self
                .observation_timepoints_day
                .windows(2)
                .any(|pair| pair[1] <= pair[0] || pair[1] - pair[0] != self.interval_days)
            || self.control.coefficients.len() != states * states
            || self.treatment.coefficients.len() != states * states
            || self.control.source_state_summaries.len() != states
            || self.treatment.source_state_summaries.len() != states
            || self.contrasts.len() != states * states
            || self.batch_sensitivity.len() != states
            || self.held_out_forecasts.len() != 2
            || self.bootstrap_draws.len() != self.bootstrap_replicates
            || self
                .bootstrap_draws
                .iter()
                .enumerate()
                .any(|(index, draw)| {
                    usize::from(draw.replicate_index) != index
                        || draw.control_coefficients_ppm.len() != states * states
                        || draw.treatment_coefficients_ppm.len() != states * states
                        || draw
                            .control_coefficients_ppm
                            .iter()
                            .chain(&draw.treatment_coefficients_ppm)
                            .any(|coefficient| *coefficient > self.max_coefficient_ppm)
                })
            || self.control.arm_id != self.control_arm
            || self.treatment.arm_id != self.treatment_arm
            || self.control_arm == self.treatment_arm
            || !(MIN_BOOTSTRAP..=MAX_BOOTSTRAP).contains(&self.bootstrap_replicates)
            || self.ridge_penalty_ppm > 1_000_000
            || self.max_coefficient_ppm == 0
            || self.max_coefficient_ppm > MAX_OPERATOR_PPM
            || self.max_prediction_error_ppm > 1_000_000
            || !(800..=999).contains(&self.confidence_level_milli)
            || self.minimum_effect_ppm == 0
            || self.minimum_effect_ppm > self.max_coefficient_ppm
            || self
                .control
                .coefficients
                .iter()
                .any(|coefficient| coefficient.coefficient_ppm > self.max_coefficient_ppm)
            || self
                .treatment
                .coefficients
                .iter()
                .any(|coefficient| coefficient.coefficient_ppm > self.max_coefficient_ppm)
            || self
                .control
                .coefficients
                .iter()
                .enumerate()
                .any(|(index, coefficient)| {
                    coefficient.from_state != self.state_order[index % states]
                        || coefficient.to_state != self.state_order[index / states]
                })
            || self
                .treatment
                .coefficients
                .iter()
                .enumerate()
                .any(|(index, coefficient)| {
                    coefficient.from_state != self.state_order[index % states]
                        || coefficient.to_state != self.state_order[index / states]
                })
            || self.contrasts.windows(2).any(|pair| {
                (&pair[0].from_state, &pair[0].to_state) >= (&pair[1].from_state, &pair[1].to_state)
            })
        {
            return Err(LineagePropagationError::InvalidOutput(
                "feature/schema, schedule, matrix dimensions, arms, bootstrap, or contrast ordering is invalid".into(),
            ));
        }
        for (index, sensitivity) in self.batch_sensitivity.iter().enumerate() {
            let expected_gap = if sensitivity.batch_count == 0 {
                1_000
            } else {
                ((u64::from(
                    sensitivity
                        .batch_count
                        .saturating_sub(sensitivity.evaluated_batch_count),
                ) * 1_000)
                    / u64::from(sensitivity.batch_count)) as u16
            };
            let unstable = !sensitivity.unstable_contrast_order.is_empty()
                || sensitivity.direction_reversal_count > 0;
            if sensitivity.from_state != self.state_order[index]
                || sensitivity.evaluated_batch_count > sensitivity.batch_count
                || sensitivity.robustness_gap_milli != expected_gap
                || sensitivity.maximum_leave_one_batch_out_deviation_ppm
                    > self.max_coefficient_ppm.saturating_mul(2)
                || sensitivity.direction_reversal_count
                    > sensitivity
                        .evaluated_batch_count
                        .saturating_mul(states as u32)
                || sensitivity.rationale.trim().is_empty()
                || sensitivity
                    .unstable_contrast_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || sensitivity.unstable_contrast_order.iter().any(|key| {
                    !self
                        .state_order
                        .iter()
                        .any(|to_state| key == &format!("{}:{}", sensitivity.from_state, to_state))
                })
                || match sensitivity.disposition {
                    LineagePropagationBatchSensitivityDisposition::Stable => {
                        sensitivity.batch_count < 2
                            || sensitivity.evaluated_batch_count != sensitivity.batch_count
                            || sensitivity.robustness_gap_milli != 0
                            || unstable
                    }
                    LineagePropagationBatchSensitivityDisposition::Fragile => {
                        sensitivity.evaluated_batch_count == 0 || !unstable
                    }
                    LineagePropagationBatchSensitivityDisposition::Partial => {
                        sensitivity.evaluated_batch_count == 0
                            || sensitivity.evaluated_batch_count == sensitivity.batch_count
                            || unstable
                    }
                    LineagePropagationBatchSensitivityDisposition::Unresolved => {
                        sensitivity.evaluated_batch_count != 0
                    }
                }
            {
                return Err(LineagePropagationError::InvalidOutput(
                    "batch-sensitivity rows are not canonical or their fold/gap disposition is inconsistent".into(),
                ));
            }
        }
        let digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LineagePropagationError::Digest(error.to_string()))?;
        if digest != self.digest {
            return Err(LineagePropagationError::InvalidOutput(
                "lineage propagation digest does not match output".into(),
            ));
        }
        Ok(())
    }
}

/// Infer arm-specific finite-interval state propagation and score the final interval as held out.
pub fn analyze_glioma_lineage_propagation(
    request: &LineagePropagationRequest,
    snapshots: &[LineagePropagationSnapshot],
) -> Result<LineagePropagationAnalysis, LineagePropagationError> {
    validate_request(request)?;
    let (trajectories, timepoints, incomplete_lineage_count) =
        validate_and_prepare(request, snapshots)?;
    let states = request.state_order.len();
    let mut rng = SplitMix64::from_hash(&request.bootstrap_seed);
    let mut outputs = Vec::new();
    let mut fits_by_arm = BTreeMap::<String, Vec<Fit>>::new();
    let mut full_by_arm = BTreeMap::<String, Fit>::new();
    let mut training_rows_by_arm = BTreeMap::<String, Vec<FitRow>>::new();
    let mut zero_source_interval_count = 0_u32;
    for arm in [&request.control_arm, &request.treatment_arm] {
        let arm_trajectories = trajectories
            .iter()
            .filter(|trajectory| &trajectory.arm == arm)
            .cloned()
            .collect::<Vec<_>>();
        let (_, zero) = make_rows(&arm_trajectories, true);
        zero_source_interval_count = zero_source_interval_count.saturating_add(zero);
        let training_rows = arm_rows(&trajectories, arm, true);
        let forecast_rows = arm_rows(&trajectories, arm, false);
        let units = arm_trajectories
            .iter()
            .map(|trajectory| trajectory.unit.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let lineages = arm_trajectories.len();
        let work = (request.bootstrap_replicates as u128)
            * (states as u128)
            * (states as u128)
            * ((training_rows.len() + MAX_SWEEPS) as u128);
        if work > MAX_BOOTSTRAP_WORK {
            return Err(LineagePropagationError::InvalidRequest(format!(
                "bounded clustered bootstrap work would require {work} operations, above {MAX_BOOTSTRAP_WORK}"
            )));
        }
        let (samples, full) = bootstrap_fits(&training_rows, request, &mut rng);
        let unit_set = arm_trajectories
            .iter()
            .map(|trajectory| trajectory.unit.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let operator = build_operator(
            arm,
            &request.state_order,
            &samples,
            &full,
            units,
            lineages,
            training_rows.len(),
            request.confidence_level_milli,
            request.max_coefficient_ppm,
        );
        // Keep this assertion close to the fitting boundary: every eligible replicate must
        // actually contribute at least one pre-holdout source observation.
        let weighted = unit_set
            .iter()
            .filter(|unit| training_rows.iter().any(|row| &row.unit == *unit))
            .count();
        if weighted < request.min_units_per_arm
            || training_rows.is_empty()
            || forecast_rows.is_empty()
        {
            return Err(LineagePropagationError::InvalidInput(format!(
                "arm {arm} lacks independent units represented in training and held-out intervals"
            )));
        }
        outputs.push(operator);
        training_rows_by_arm.insert(arm.clone(), training_rows);
        fits_by_arm.insert(arm.clone(), samples);
        full_by_arm.insert(arm.clone(), full);
    }
    let batch_sensitivity = batch_sensitivity(
        request,
        &request.state_order,
        &training_rows_by_arm,
        &full_by_arm,
    );
    let control_samples = &fits_by_arm[&request.control_arm];
    let treatment_samples = &fits_by_arm[&request.treatment_arm];
    let control_full = &full_by_arm[&request.control_arm];
    let treatment_full = &full_by_arm[&request.treatment_arm];
    let bootstrap_draws = control_samples
        .iter()
        .zip(treatment_samples)
        .enumerate()
        .map(
            |(index, (control, treatment))| LineagePropagationBootstrapDraw {
                replicate_index: index as u16,
                control_coefficients_ppm: control.matrix.clone(),
                treatment_coefficients_ppm: treatment.matrix.clone(),
            },
        )
        .collect::<Vec<_>>();
    let mut contrasts = Vec::with_capacity(states * states);
    for to in 0..states {
        for from in 0..states {
            let index = to * states + from;
            let effect = treatment_full.matrix[index] as i64 - control_full.matrix[index] as i64;
            let interval = quantile_interval(
                treatment_samples
                    .iter()
                    .zip(control_samples)
                    .map(|(treatment, control)| {
                        treatment.matrix[index] as i64 - control.matrix[index] as i64
                    })
                    .collect(),
                request.confidence_level_milli,
            );
            let margin = request.minimum_effect_ppm as i64;
            let disposition = if control_full.rank < states || treatment_full.rank < states {
                EffectDisposition::Unresolved
            } else if interval.lower > margin || interval.upper < -margin {
                EffectDisposition::BeyondPracticalMargin
            } else if interval.lower >= -margin && interval.upper <= margin {
                EffectDisposition::WithinPracticalMargin
            } else {
                EffectDisposition::Inconclusive
            };
            contrasts.push(LineagePropagationContrast {
                from_state: request.state_order[from].clone(),
                to_state: request.state_order[to].clone(),
                treatment_minus_control_ppm: effect,
                bootstrap_interval_ppm: interval,
                disposition,
            });
        }
    }
    contrasts.sort_by(|left, right| {
        (&left.from_state, &left.to_state).cmp(&(&right.from_state, &right.to_state))
    });
    let mut held_out_forecasts = vec![
        forecast(
            &trajectories,
            &request.control_arm,
            &control_full.matrix,
            timepoints[1] - timepoints[0],
            states,
        ),
        forecast(
            &trajectories,
            &request.treatment_arm,
            &treatment_full.matrix,
            timepoints[1] - timepoints[0],
            states,
        ),
    ];
    held_out_forecasts.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    let rank_deficient = outputs
        .iter()
        .any(|operator| usize::from(operator.source_design_rank) < states);
    let poor_prediction = held_out_forecasts.iter().any(|forecast| {
        forecast.mean_composition_error_ppm > request.max_prediction_error_ppm
            || forecast.mean_abundance_error_ppm > request.max_prediction_error_ppm
    });
    let boundary_or_nonconvergence = outputs
        .iter()
        .any(|operator| operator.boundary_coefficient_count > 0 || !operator.converged);
    let disposition = if rank_deficient {
        LineagePropagationDisposition::Unresolved
    } else if poor_prediction {
        LineagePropagationDisposition::PredictionFailure
    } else if boundary_or_nonconvergence {
        LineagePropagationDisposition::PartiallyIdentified
    } else {
        LineagePropagationDisposition::Qualified
    };
    let mut limitations = vec![
        "finite-interval propagation coefficients combine switching, proliferation, death, and state-assignment effects; they are not direct cell transition rates".into(),
        "capture-fraction correction is treated as known; uncertainty in capture estimation is not propagated".into(),
        "corrected count noise and barcode-specific measurement error are not separately modeled".into(),
        "complete barcode trajectories are analyzed; missing records are excluded rather than interpreted as extinction".into(),
        "the final scheduled interval is a temporal holdout, while bootstrap intervals resample independent experimental units, not cells or barcode rows".into(),
    ];
    if incomplete_lineage_count > 0 {
        limitations.push(format!(
            "{incomplete_lineage_count} incomplete barcode trajectories were excluded"
        ));
    }
    if zero_source_interval_count > 0 {
        limitations.push(format!(
            "{zero_source_interval_count} zero-source intervals were excluded from fitting"
        ));
    }
    if batch_sensitivity.iter().any(|row| {
        matches!(
            row.disposition,
            LineagePropagationBatchSensitivityDisposition::Fragile
                | LineagePropagationBatchSensitivityDisposition::Partial
                | LineagePropagationBatchSensitivityDisposition::Unresolved
        )
    }) {
        limitations.push(
            "assay-batch leave-one-out sensitivity was fragile, partial, or unresolved; batch robustness is not established".into(),
        );
    }
    let mut canonical_snapshots = snapshots.to_vec();
    canonical_snapshots.sort_by(|left, right| {
        (
            &left.arm_id,
            &left.experimental_unit_id,
            &left.lineage_id,
            left.timepoint_day,
        )
            .cmp(&(
                &right.arm_id,
                &right.experimental_unit_id,
                &right.lineage_id,
                right.timepoint_day,
            ))
    });
    let input_digest = ContentHash::of_value(&serde_json::json!({
        "request": request,
        "snapshots": canonical_snapshots,
    }))
    .map_err(|error| LineagePropagationError::Digest(error.to_string()))?;
    let mut output = LineagePropagationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        state_order: request.state_order.clone(),
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        observation_timepoints_day: timepoints.clone(),
        interval_days: timepoints[1] - timepoints[0],
        control: outputs.remove(0),
        treatment: outputs.remove(0),
        contrasts,
        batch_sensitivity,
        held_out_forecasts,
        incomplete_lineage_count,
        zero_source_interval_count,
        ridge_penalty_ppm: request.ridge_penalty_ppm,
        max_coefficient_ppm: request.max_coefficient_ppm,
        max_prediction_error_ppm: request.max_prediction_error_ppm,
        bootstrap_replicates: request.bootstrap_replicates,
        bootstrap_draws,
        bootstrap_seed: request.bootstrap_seed.clone(),
        confidence_level_milli: request.confidence_level_milli,
        minimum_effect_ppm: request.minimum_effect_ppm,
        disposition,
        limitations,
        input_digest,
        digest: ContentHash::of_bytes(b"unsealed-glioma-lineage-propagation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LineagePropagationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> LineagePropagationRequest {
        LineagePropagationRequest {
            objective: "compare effective glioma state propagation in preclinical cultures".into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["npc_like".into(), "mes_like".into()],
            min_units_per_arm: 2,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 1,
            max_coefficient_ppm: 5_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"lineage-propagation-seed"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        }
    }

    fn snapshots() -> Vec<LineagePropagationSnapshot> {
        snapshots_with_treatment_matrices([[[6, 4], [3, 5]]; 3])
    }

    fn snapshots_with_treatment_matrices(
        treatment_matrices: [[[u32; 2]; 2]; 3],
    ) -> Vec<LineagePropagationSnapshot> {
        let mut rows = Vec::new();
        for arm in ["control", "perturbation"] {
            for (unit_index, treatment_matrix) in treatment_matrices.iter().enumerate() {
                let matrix = if arm == "control" {
                    [[8, 2], [1, 7]]
                } else {
                    *treatment_matrix
                };
                for source in 0..2 {
                    let lineage = format!("{arm}-u{unit_index}-l{source}");
                    let mut values = vec![0_u32; 2];
                    values[source] = 100;
                    for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                        rows.push(LineagePropagationSnapshot {
                            observation_id: format!("{lineage}-t{day}"),
                            experimental_unit_id: format!("{arm}-u{unit_index}"),
                            lineage_id: lineage.clone(),
                            arm_id: arm.into(),
                            model_system: GliomaModelSystem::Organoid,
                            assay_batch_id: format!("assay-batch-{unit_index}"),
                            timepoint_day: day,
                            state_counts: values.clone(),
                            capture_fraction_ppm: 1_000_000,
                            artifact: artifact(&format!("{lineage}-{day}")),
                        });
                        if step < 2 {
                            values = (0..2)
                                .map(|to| {
                                    (0..2)
                                        .map(|from| matrix[to][from] * values[from])
                                        .sum::<u32>()
                                        / 10
                                })
                                .collect();
                        }
                    }
                }
            }
        }
        rows
    }

    #[test]
    fn recovers_different_finite_interval_operators_and_scores_held_out_data() {
        let result = analyze_glioma_lineage_propagation(&request(), &snapshots()).unwrap();
        assert_eq!(result.control.source_design_rank, 2);
        assert_eq!(result.treatment.source_design_rank, 2);
        assert_eq!(result.control.training_pair_count, 6);
        assert_eq!(result.treatment.training_pair_count, 6);
        let control = result
            .control
            .coefficients
            .iter()
            .find(|coefficient| {
                coefficient.from_state == "npc_like" && coefficient.to_state == "npc_like"
            })
            .unwrap();
        let treatment = result
            .treatment
            .coefficients
            .iter()
            .find(|coefficient| {
                coefficient.from_state == "npc_like" && coefficient.to_state == "npc_like"
            })
            .unwrap();
        assert!(control.coefficient_ppm.abs_diff(800_000) < 10_000);
        assert!(treatment.coefficient_ppm.abs_diff(600_000) < 10_000);
        assert!(result
            .held_out_forecasts
            .iter()
            .all(|forecast| forecast.mean_composition_error_ppm < 10_000));
        assert!(result
            .held_out_forecasts
            .iter()
            .all(|forecast| forecast.mean_abundance_error_ppm < 10_000));
        assert!(result.batch_sensitivity.iter().all(|sensitivity| {
            sensitivity.disposition == LineagePropagationBatchSensitivityDisposition::Stable
                && sensitivity.batch_count == 3
                && sensitivity.evaluated_batch_count == 3
                && sensitivity.robustness_gap_milli == 0
        }));
        result.validate().unwrap();
    }

    #[test]
    fn leave_one_assay_batch_out_refit_flags_a_state_direction_reversal() {
        let rows = snapshots_with_treatment_matrices([
            [[10, 2], [0, 7]],
            [[10, 2], [0, 7]],
            [[2, 2], [6, 7]],
        ]);
        let output = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        let npc = output
            .batch_sensitivity
            .iter()
            .find(|row| row.from_state == "npc_like")
            .unwrap();
        assert_eq!(
            npc.disposition,
            LineagePropagationBatchSensitivityDisposition::Fragile
        );
        assert!(npc.direction_reversal_count >= 1);
        assert!(npc
            .unstable_contrast_order
            .contains(&"npc_like:mes_like".to_string()));
        assert_eq!(npc.evaluated_batch_count, 3);
        output.validate().unwrap();
    }

    #[test]
    fn incomplete_lineages_are_excluded_not_encoded_as_extinctions() {
        let mut rows = snapshots();
        rows.retain(|row| row.observation_id != "control-u0-l0-t7");
        let output = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        assert_eq!(output.incomplete_lineage_count, 1);
        assert_eq!(output.control.complete_lineage_count, 5);
    }

    #[test]
    fn rank_deficient_mixtures_do_not_receive_mechanistic_qualification() {
        let mut rows = snapshots();
        rows.retain(|row| !row.lineage_id.ends_with("-l1"));
        let mut req = request();
        req.min_lineages_per_arm = 2;
        let output = analyze_glioma_lineage_propagation(&req, &rows).unwrap();
        assert_eq!(output.control.source_design_rank, 1);
        assert_eq!(
            output.disposition,
            LineagePropagationDisposition::Unresolved
        );
        assert!(output
            .control
            .coefficients
            .iter()
            .all(|coefficient| coefficient.disposition == CoefficientDisposition::RankUnresolved));
    }

    #[test]
    fn source_unit_resampling_is_reproducible_under_input_permutation() {
        let mut rows = snapshots();
        let first = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        rows.reverse();
        let second = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn bootstrap_intervals_reflect_between_unit_variation_not_barcode_count() {
        let mut rows = snapshots();
        for row in rows.iter_mut().filter(|row| {
            row.arm_id == "control"
                && row.experimental_unit_id == "control-u0"
                && row.timepoint_day == 7
        }) {
            row.state_counts = if row.lineage_id.ends_with("-l0") {
                vec![70, 20]
            } else {
                vec![30, 60]
            };
        }
        let output = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        let coefficient = output
            .control
            .coefficients
            .iter()
            .find(|coefficient| {
                coefficient.from_state == "npc_like" && coefficient.to_state == "npc_like"
            })
            .unwrap();
        assert!(
            coefficient.bootstrap_interval_ppm.lower < coefficient.bootstrap_interval_ppm.upper
        );
        for summary in &output.control.source_state_summaries {
            assert_eq!(
                summary
                    .destination_composition
                    .iter()
                    .map(|destination| destination.share_ppm as u64)
                    .sum::<u64>(),
                SCALE
            );
        }
    }

    #[test]
    fn duplicating_barcode_profiles_within_a_culture_does_not_reweight_that_culture() {
        let mut rows = snapshots();
        for row in rows
            .iter_mut()
            .filter(|row| row.arm_id == "control" && row.experimental_unit_id == "control-u0")
        {
            if row.timepoint_day > 0 {
                row.state_counts = vec![50, 50];
            }
        }
        let baseline = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        let unit_profiles = rows
            .iter()
            .filter(|row| row.arm_id == "control" && row.experimental_unit_id == "control-u0")
            .cloned()
            .collect::<Vec<_>>();
        for duplicate in 1..50 {
            for mut row in unit_profiles.clone() {
                row.lineage_id = format!("{}-copy-{duplicate}", row.lineage_id);
                row.observation_id = format!("{}-copy-{duplicate}", row.observation_id);
                row.artifact = artifact(&row.observation_id);
                rows.push(row);
            }
        }
        let duplicated = analyze_glioma_lineage_propagation(&request(), &rows).unwrap();
        assert_eq!(
            baseline.control.coefficients,
            duplicated.control.coefficients
        );
        assert_eq!(baseline.bootstrap_draws, duplicated.bootstrap_draws);
    }

    #[test]
    fn schedule_capture_and_direct_identifier_guards_fail_closed() {
        let mut rows = snapshots();
        rows[0].capture_fraction_ppm = 0;
        assert!(matches!(
            analyze_glioma_lineage_propagation(&request(), &rows),
            Err(LineagePropagationError::InvalidInput(_))
        ));
        let mut rows = snapshots();
        rows[0].artifact.contains_direct_identifiers = true;
        assert!(matches!(
            analyze_glioma_lineage_propagation(&request(), &rows),
            Err(LineagePropagationError::InvalidInput(_))
        ));
        let mut rows = snapshots();
        rows[0].timepoint_day = 3;
        assert!(matches!(
            analyze_glioma_lineage_propagation(&request(), &rows),
            Err(LineagePropagationError::InvalidInput(_))
        ));
    }
}
