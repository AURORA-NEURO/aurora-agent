//! Cross-validated calibration of glioma assay outcomes against lineage-propagation contrasts.
//!
//! Calibration rows represent distinct preclinical experimental units. A compact, quantile-binned
//! categorical model maps a declared source→destination treatment-control propagation contrast to
//! assay outcomes. Leave-one-unit-out Brier skill gates whether that relationship is used. If it
//! does not beat a prevalence-only predictor by the configured margin, the emitted likelihoods are
//! constant across P10 bootstrap particles, so the assay cannot gain artificial acquisition value.

use super::lineage_acquisition_design::{
    LineagePropagationAssayResponseModel, LineagePropagationJointAssayResponseModel,
    LineagePropagationJointOutcomeLikelihood, LineagePropagationOutcomeLikelihood,
};
use super::state_stratified_campaign::StratifiedAssayCandidate;
use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
    LineagePropagationAnalysis, LineagePropagationDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = super::lineage_acquisition_design::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaLineageAssayResponseCalibration1@1";
pub const JOINT_OUTPUT_SCHEMA: &str = "GliomaLineageJointAssayResponseCalibration1@1";
pub const DEFAULT_MINIMUM_BRIER_SKILL_PPM: u32 = 50_000;
const PROBABILITY_MILLI: u64 = 1_000;
const MAX_CALIBRATION_UNITS: usize = 2_048;
const MAX_OUTCOMES: usize = 16;
const MAX_QUANTILE_BINS: u16 = 16;
const MAX_ABS_CONTRAST_PPM: i64 = 50_000_000;
const MAX_JOINT_CALIBRATION_WORK: u128 = 100_000_000;

/// One independent calibration unit paired to its measured assay outcome and lineage contrast.
/// The contrast is treatment-minus-control for the declared transition, estimated outside this
/// calibrator from that unit's matched preclinical measurements. Multiple barcodes or cells from
/// the same unit must remain summarized in one row; they are not independent calibration samples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayCalibrationObservation {
    pub experimental_unit_id: String,
    pub treatment_minus_control_coefficient_ppm: i64,
    pub outcome_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayResponseCalibrationRequest {
    /// Destination state whose transition from the candidate's source stratum is calibrated.
    pub destination_state_id: String,
    pub model_version: String,
    pub maximum_quantile_bins: u16,
    /// Minimum leave-one-unit-out Brier skill over a prevalence-only predictor, in ppm.
    /// 50,000 means the binned model must reduce Brier score by at least 5%.
    pub minimum_brier_skill_ppm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageAssayResponseCalibrationDisposition {
    Predictive,
    BaselineOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayResponseCalibrationDiagnostics {
    pub calibration_unit_count: u32,
    pub outcome_count: u16,
    pub minimum_units_per_outcome: u32,
    pub cross_validation_fold_count: u16,
    pub selected_quantile_bin_count: u16,
    /// Multiclass Brier score scaled by 1,000,000; lower is better.
    pub leave_one_unit_out_brier_million: u64,
    /// Smoothed prevalence-only Brier score on the same held-out units.
    pub prevalence_baseline_brier_million: u64,
    /// Relative Brier improvement, where 50,000 is 5%; negative means the model was worse.
    pub brier_skill_ppm: i64,
    /// Bootstrap particles outside observed calibration contrast support receive baseline-only
    /// likelihoods rather than extrapolated tail probabilities.
    pub out_of_calibration_support_draw_count: u32,
    pub calibration_contrast_min_ppm: i64,
    pub calibration_contrast_max_ppm: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayResponseCalibrationRun {
    pub feature_id: String,
    pub output_schema: String,
    pub action_id: String,
    pub source_state_id: String,
    pub destination_state_id: String,
    pub analysis_digest: ContentHash,
    pub disposition: LineageAssayResponseCalibrationDisposition,
    pub response_model: LineagePropagationAssayResponseModel,
    pub diagnostics: LineageAssayResponseCalibrationDiagnostics,
    pub digest: ContentHash,
}

/// Paired measurements from one independent preclinical experimental unit. The order of assay
/// outcomes corresponds to the two candidate arguments after lexical action-ID normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayJointCalibrationObservation {
    pub experimental_unit_id: String,
    pub treatment_minus_control_coefficient_ppm: i64,
    pub first_outcome_id: String,
    pub second_outcome_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayJointResponseCalibrationRequest {
    pub destination_state_id: String,
    pub model_version: String,
    pub maximum_quantile_bins: u16,
    /// Minimum marginal leave-one-unit-out Brier skill over prevalence-only predictors, in ppm.
    pub minimum_marginal_brier_skill_ppm: u32,
    /// Minimum joint leave-one-unit-out Brier skill over the product-of-marginals baseline, in ppm.
    pub minimum_dependence_brier_skill_ppm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageAssayJointResponseCalibrationDisposition {
    ConditionalDependenceSupported,
    ConditionalIndependenceBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayJointResponseCalibrationDiagnostics {
    pub calibration_unit_count: u32,
    pub joint_outcome_count: u16,
    pub minimum_units_per_joint_outcome: u32,
    pub cross_validation_fold_count: u16,
    pub selected_quantile_bin_count: u16,
    /// Joint multiclass Brier loss scaled by 1,000,000 after marginal projection.
    pub leave_one_unit_out_brier_million: u64,
    /// Product-of-marginals conditional-independence Brier loss on identical held-out units.
    pub independence_baseline_brier_million: u64,
    /// Relative improvement over conditional independence; negative values favor independence.
    pub brier_skill_ppm: i64,
    /// Draws outside paired calibration support use the calibrated single-assay product model.
    pub out_of_calibration_support_draw_count: u32,
    pub calibration_contrast_min_ppm: i64,
    pub calibration_contrast_max_ppm: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAssayJointResponseCalibrationRun {
    pub feature_id: String,
    pub output_schema: String,
    pub first_action_id: String,
    pub second_action_id: String,
    pub source_state_id: String,
    pub destination_state_id: String,
    pub analysis_digest: ContentHash,
    pub disposition: LineageAssayJointResponseCalibrationDisposition,
    pub first_response_model: LineagePropagationAssayResponseModel,
    pub second_response_model: LineagePropagationAssayResponseModel,
    pub joint_response_model: LineagePropagationJointAssayResponseModel,
    pub diagnostics: LineageAssayJointResponseCalibrationDiagnostics,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineageAssayResponseCalibrationError {
    #[error("lineage assay response calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage assay response calibration input is invalid: {0}")]
    InvalidInput(String),
    #[error("lineage assay calibration evidence is insufficient: {0}")]
    InsufficientEvidence(String),
    #[error("lineage assay response calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage assay response calibration digest failed: {0}")]
    Digest(String),
}

fn calibration_digest_input(
    request: &LineageAssayResponseCalibrationRequest,
    candidate: &StratifiedAssayCandidate,
    observations: &[LineageAssayCalibrationObservation],
) -> serde_json::Value {
    serde_json::json!({
        "action_id": candidate.action.action_id,
        "source_state_id": candidate.stratum_id,
        "candidate_outcomes": candidate.action.outcomes,
        "request": request,
        "observations": observations,
    })
}

fn run_digest_input(run: &LineageAssayResponseCalibrationRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "action_id": run.action_id,
        "source_state_id": run.source_state_id,
        "destination_state_id": run.destination_state_id,
        "analysis_digest": run.analysis_digest,
        "disposition": run.disposition,
        "response_model": run.response_model,
        "diagnostics": run.diagnostics,
    })
}

fn joint_calibration_digest_input(
    request: &LineageAssayJointResponseCalibrationRequest,
    first_candidate: &StratifiedAssayCandidate,
    second_candidate: &StratifiedAssayCandidate,
    observations: &[LineageAssayJointCalibrationObservation],
) -> serde_json::Value {
    serde_json::json!({
        "request": request,
        "first_action": first_candidate.action,
        "first_source_state_id": first_candidate.stratum_id,
        "second_action": second_candidate.action,
        "second_source_state_id": second_candidate.stratum_id,
        "observations": observations,
    })
}

fn joint_run_digest_input(run: &LineageAssayJointResponseCalibrationRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "first_action_id": run.first_action_id,
        "second_action_id": run.second_action_id,
        "source_state_id": run.source_state_id,
        "destination_state_id": run.destination_state_id,
        "analysis_digest": run.analysis_digest,
        "disposition": run.disposition,
        "first_response_model": run.first_response_model,
        "second_response_model": run.second_response_model,
        "joint_response_model": run.joint_response_model,
        "diagnostics": run.diagnostics,
    })
}

fn outcome_ids(
    candidate: &StratifiedAssayCandidate,
) -> Result<Vec<String>, LineageAssayResponseCalibrationError> {
    let ids = candidate
        .action
        .outcomes
        .iter()
        .map(|outcome| outcome.outcome_id.clone())
        .collect::<Vec<_>>();
    let unique = ids.iter().collect::<BTreeSet<_>>();
    if candidate.action.action_id.trim().is_empty()
        || candidate.stratum_id.trim().is_empty()
        || candidate.action.cost_units == 0
        || ids.len() < 2
        || ids.len() > MAX_OUTCOMES
        || unique.len() != ids.len()
        || ids.iter().any(|id| id.trim().is_empty())
    {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "candidate identity, nonzero cost, and 2..=16 unique outcome IDs are required".into(),
        ));
    }
    Ok(ids)
}

fn validate_inputs(
    analysis: &LineagePropagationAnalysis,
    candidate: &StratifiedAssayCandidate,
    request: &LineageAssayResponseCalibrationRequest,
    observations: &[LineageAssayCalibrationObservation],
) -> Result<Vec<String>, LineageAssayResponseCalibrationError> {
    let outcomes = outcome_ids(candidate)?;
    if analysis.disposition != LineagePropagationDisposition::Qualified {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "calibration requires a qualified P10 lineage-propagation analysis".into(),
        ));
    }
    analysis
        .validate()
        .map_err(|error| LineageAssayResponseCalibrationError::InvalidRequest(error.to_string()))?;
    if !analysis.state_order.contains(&candidate.stratum_id)
        || !analysis.state_order.contains(&request.destination_state_id)
        || request.destination_state_id.trim().is_empty()
        || request.model_version.trim().is_empty()
        || request.maximum_quantile_bins == 0
        || request.maximum_quantile_bins > MAX_QUANTILE_BINS
        || request.minimum_brier_skill_ppm > 1_000_000
    {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "source/destination states, model version, bin count, or Brier-skill threshold is invalid".into(),
        ));
    }
    if observations.len() < 12 || observations.len() > MAX_CALIBRATION_UNITS {
        return Err(LineageAssayResponseCalibrationError::InsufficientEvidence(
            format!("12..={MAX_CALIBRATION_UNITS} independent experimental units are required"),
        ));
    }
    let declared_outcomes = outcomes.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut units = BTreeSet::new();
    let mut counts = outcomes
        .iter()
        .map(|outcome| (outcome.as_str(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    for observation in observations {
        if observation.experimental_unit_id.trim().is_empty()
            || observation.treatment_minus_control_coefficient_ppm.abs() > MAX_ABS_CONTRAST_PPM
            || !declared_outcomes.contains(observation.outcome_id.as_str())
            || !units.insert(observation.experimental_unit_id.as_str())
        {
            return Err(LineageAssayResponseCalibrationError::InvalidRequest(
                "each row must be one unique independent unit with an in-range contrast and declared outcome".into(),
            ));
        }
        *counts
            .get_mut(observation.outcome_id.as_str())
            .expect("declared outcome was checked") += 1;
    }
    if let Some((outcome, count)) = counts.iter().find(|(_, count)| **count < 3) {
        return Err(LineageAssayResponseCalibrationError::InsufficientEvidence(
            format!("outcome {outcome} has {count} independent units; at least 3 are required"),
        ));
    }
    Ok(outcomes)
}

fn probability_milli(counts: &[u32]) -> Vec<u16> {
    let smoothed = counts
        .iter()
        .map(|count| u64::from(*count) + 1)
        .collect::<Vec<_>>();
    let denominator = smoothed.iter().sum::<u64>();
    let mut probabilities = smoothed
        .iter()
        .map(|count| (count * PROBABILITY_MILLI / denominator) as u16)
        .collect::<Vec<_>>();
    let used = probabilities
        .iter()
        .map(|value| u64::from(*value))
        .sum::<u64>();
    let mut remainder_order = smoothed
        .iter()
        .enumerate()
        .map(|(index, count)| (index, count * PROBABILITY_MILLI % denominator))
        .collect::<Vec<_>>();
    remainder_order.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (index, _) in remainder_order
        .into_iter()
        .take((PROBABILITY_MILLI - used) as usize)
    {
        probabilities[index] += 1;
    }
    probabilities
}

fn outcome_counts(
    observations: &[LineageAssayCalibrationObservation],
    outcomes: &[String],
) -> Vec<u32> {
    outcomes
        .iter()
        .map(|outcome| {
            observations
                .iter()
                .filter(|observation| observation.outcome_id == *outcome)
                .count() as u32
        })
        .collect()
}

fn quantile_cutpoints(
    observations: &[LineageAssayCalibrationObservation],
    maximum_bins: u16,
) -> Vec<i64> {
    if observations.len() < 8 || maximum_bins < 2 {
        return Vec::new();
    }
    let bin_count = (observations.len() / 4)
        .max(1)
        .min(usize::from(maximum_bins));
    if bin_count <= 1 {
        return Vec::new();
    }
    let mut values = observations
        .iter()
        .map(|observation| observation.treatment_minus_control_coefficient_ppm)
        .collect::<Vec<_>>();
    values.sort_unstable();
    let mut cuts = Vec::with_capacity(bin_count - 1);
    for bin in 1..bin_count {
        let split = bin * values.len() / bin_count;
        if split == 0 || split >= values.len() || values[split - 1] == values[split] {
            continue;
        }
        let lower = i128::from(values[split - 1]);
        let upper = i128::from(values[split]);
        cuts.push((lower + (upper - lower) / 2) as i64);
    }
    cuts.dedup();
    cuts
}

fn bin_index(cutpoints: &[i64], value: i64) -> usize {
    cutpoints.partition_point(|cutpoint| value > *cutpoint)
}

#[derive(Debug, Clone)]
struct BinnedOutcomeModel {
    cutpoints: Vec<i64>,
    outcome_counts_by_bin: Vec<Vec<u32>>,
}

impl BinnedOutcomeModel {
    fn fit(
        training: &[LineageAssayCalibrationObservation],
        outcomes: &[String],
        maximum_bins: u16,
    ) -> Self {
        let cutpoints = quantile_cutpoints(training, maximum_bins);
        let mut outcome_counts_by_bin = vec![vec![0_u32; outcomes.len()]; cutpoints.len() + 1];
        for observation in training {
            let bin = bin_index(
                &cutpoints,
                observation.treatment_minus_control_coefficient_ppm,
            );
            let outcome = outcomes
                .iter()
                .position(|outcome| outcome == &observation.outcome_id)
                .expect("training outcome was validated");
            outcome_counts_by_bin[bin][outcome] += 1;
        }
        Self {
            cutpoints,
            outcome_counts_by_bin,
        }
    }

    fn predict(&self, value: i64) -> Vec<u16> {
        probability_milli(&self.outcome_counts_by_bin[bin_index(&self.cutpoints, value)])
    }
}

#[derive(Debug, Clone)]
struct FlowEdge {
    to: usize,
    reverse: usize,
    capacity: u16,
}

fn add_flow_edge(graph: &mut [Vec<FlowEdge>], from: usize, to: usize, capacity: u16) -> usize {
    let forward_index = graph[from].len();
    let reverse_index = graph[to].len();
    graph[from].push(FlowEdge {
        to,
        reverse: reverse_index,
        capacity,
    });
    graph[to].push(FlowEdge {
        to: from,
        reverse: forward_index,
        capacity: 0,
    });
    forward_index
}

fn flow_levels(graph: &[Vec<FlowEdge>], source: usize, sink: usize) -> Option<Vec<i32>> {
    let mut levels = vec![-1; graph.len()];
    let mut queue = VecDeque::from([source]);
    levels[source] = 0;
    while let Some(node) = queue.pop_front() {
        for edge in &graph[node] {
            if edge.capacity > 0 && levels[edge.to] < 0 {
                levels[edge.to] = levels[node] + 1;
                queue.push_back(edge.to);
            }
        }
    }
    (levels[sink] >= 0).then_some(levels)
}

fn flow_dfs(
    graph: &mut [Vec<FlowEdge>],
    levels: &[i32],
    next_edge: &mut [usize],
    node: usize,
    sink: usize,
    limit: u16,
) -> u16 {
    if node == sink {
        return limit;
    }
    while next_edge[node] < graph[node].len() {
        let edge_index = next_edge[node];
        let to = graph[node][edge_index].to;
        let reverse = graph[node][edge_index].reverse;
        let capacity = graph[node][edge_index].capacity;
        if capacity > 0 && levels[to] == levels[node] + 1 {
            let pushed = flow_dfs(graph, levels, next_edge, to, sink, limit.min(capacity));
            if pushed > 0 {
                graph[node][edge_index].capacity -= pushed;
                graph[to][reverse].capacity += pushed;
                return pushed;
            }
        }
        next_edge[node] += 1;
    }
    0
}

/// Deterministically project a positive joint table to exact integer marginals. IPF estimates a
/// real-valued table; bipartite controlled rounding then preserves both margins exactly at the
/// public 1,000-milli precision instead of independently rounding cells and changing the model.
fn project_joint_probabilities(
    initial: &[u16],
    row_marginals: &[u16],
    column_marginals: &[u16],
) -> Result<Vec<u16>, LineageAssayResponseCalibrationError> {
    let rows = row_marginals.len();
    let columns = column_marginals.len();
    if rows == 0
        || columns == 0
        || initial.len() != rows.saturating_mul(columns)
        || row_marginals
            .iter()
            .map(|value| u32::from(*value))
            .sum::<u32>()
            != 1_000
        || column_marginals
            .iter()
            .map(|value| u32::from(*value))
            .sum::<u32>()
            != 1_000
    {
        return Err(LineageAssayResponseCalibrationError::InvalidInput(
            "joint-table projection requires a normalized Cartesian table and two normalized marginals".into(),
        ));
    }
    let mut table = initial
        .iter()
        .map(|probability| f64::from(*probability))
        .collect::<Vec<_>>();
    for _ in 0..256 {
        for (row, &row_marginal) in row_marginals.iter().enumerate() {
            let start = row * columns;
            let total = table[start..start + columns].iter().sum::<f64>();
            if total <= 0.0 {
                return Err(LineageAssayResponseCalibrationError::InvalidInput(
                    "joint-table projection encountered an empty row".into(),
                ));
            }
            let scale = f64::from(row_marginal) / total;
            for value in &mut table[start..start + columns] {
                *value *= scale;
            }
        }
        for column in 0..columns {
            let total = (0..rows)
                .map(|row| table[row * columns + column])
                .sum::<f64>();
            if total <= 0.0 {
                return Err(LineageAssayResponseCalibrationError::InvalidInput(
                    "joint-table projection encountered an empty column".into(),
                ));
            }
            let scale = f64::from(column_marginals[column]) / total;
            for row in 0..rows {
                table[row * columns + column] *= scale;
            }
        }
        let maximum_row_error = (0..rows)
            .map(|row| {
                let total = table[row * columns..(row + 1) * columns]
                    .iter()
                    .sum::<f64>();
                (total - f64::from(row_marginals[row])).abs()
            })
            .fold(0.0_f64, f64::max);
        if maximum_row_error <= 1e-9 {
            break;
        }
    }

    let mut integer_table = vec![0_u16; table.len()];
    let mut fractions = vec![0.0_f64; table.len()];
    let mut row_residuals = vec![0_u16; rows];
    let mut column_residuals = vec![0_u16; columns];
    for row in 0..rows {
        let mut used = 0_u16;
        for column in 0..columns {
            let index = row * columns + column;
            let floor = table[index].floor().clamp(0.0, 1_000.0) as u16;
            integer_table[index] = floor;
            fractions[index] = table[index] - f64::from(floor);
            used = used.saturating_add(floor);
        }
        if used > row_marginals[row] {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "controlled rounding exceeded a requested row marginal".into(),
            ));
        }
        row_residuals[row] = row_marginals[row] - used;
    }
    for column in 0..columns {
        let used = (0..rows)
            .map(|row| integer_table[row * columns + column])
            .fold(0_u16, u16::saturating_add);
        if used > column_marginals[column] {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "controlled rounding exceeded a requested column marginal".into(),
            ));
        }
        column_residuals[column] = column_marginals[column] - used;
    }
    let required_flow = row_residuals
        .iter()
        .map(|value| u32::from(*value))
        .sum::<u32>();
    if required_flow
        != column_residuals
            .iter()
            .map(|value| u32::from(*value))
            .sum::<u32>()
    {
        return Err(LineageAssayResponseCalibrationError::InvalidOutput(
            "controlled-rounding residual margins are inconsistent".into(),
        ));
    }
    if required_flow > 0 {
        let source = 0;
        let first_row = 1;
        let first_column = first_row + rows;
        let sink = first_column + columns;
        let mut graph = vec![Vec::new(); sink + 1];
        for (row, demand) in row_residuals.iter().enumerate() {
            add_flow_edge(&mut graph, source, first_row + row, *demand);
        }
        let mut cell_edges = vec![vec![0_usize; columns]; rows];
        for row in 0..rows {
            let mut order = (0..columns).collect::<Vec<_>>();
            order.sort_by(|left, right| {
                fractions[row * columns + *right]
                    .total_cmp(&fractions[row * columns + *left])
                    .then_with(|| left.cmp(right))
            });
            for column in order {
                cell_edges[row][column] =
                    add_flow_edge(&mut graph, first_row + row, first_column + column, 1);
            }
        }
        for (column, demand) in column_residuals.iter().enumerate() {
            add_flow_edge(&mut graph, first_column + column, sink, *demand);
        }
        let mut flow = 0_u32;
        while let Some(levels) = flow_levels(&graph, source, sink) {
            let mut next_edge = vec![0; graph.len()];
            loop {
                let pushed = flow_dfs(&mut graph, &levels, &mut next_edge, source, sink, u16::MAX);
                if pushed == 0 {
                    break;
                }
                flow += u32::from(pushed);
            }
        }
        if flow != required_flow {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "controlled rounding could not satisfy both assay marginals".into(),
            ));
        }
        for row in 0..rows {
            for column in 0..columns {
                if graph[first_row + row][cell_edges[row][column]].capacity == 0 {
                    integer_table[row * columns + column] += 1;
                }
            }
        }
    }
    if (0..rows).any(|row| {
        integer_table[row * columns..(row + 1) * columns]
            .iter()
            .map(|value| u32::from(*value))
            .sum::<u32>()
            != u32::from(row_marginals[row])
    }) || (0..columns).any(|column| {
        (0..rows)
            .map(|row| u32::from(integer_table[row * columns + column]))
            .sum::<u32>()
            != u32::from(column_marginals[column])
    }) {
        return Err(LineageAssayResponseCalibrationError::InvalidOutput(
            "controlled-rounded joint table does not preserve exact single-assay marginals".into(),
        ));
    }
    Ok(integer_table)
}

fn independent_joint_probabilities(
    row_marginals: &[u16],
    column_marginals: &[u16],
) -> Result<Vec<u16>, LineageAssayResponseCalibrationError> {
    let initial = row_marginals
        .iter()
        .flat_map(|row| {
            column_marginals
                .iter()
                .map(move |column| ((u32::from(*row) * u32::from(*column)) / 1_000 + 1) as u16)
        })
        .collect::<Vec<_>>();
    project_joint_probabilities(&initial, row_marginals, column_marginals)
}

fn pair_outcome_key(first: &str, second: &str) -> String {
    format!("{}:{first}{}:{second}", first.len(), second.len())
}

fn projected_joint_prediction(
    model: &BinnedOutcomeModel,
    contrast: i64,
    first_outcomes: &[String],
    second_outcomes: &[String],
    first_marginals: &[u16],
    second_marginals: &[u16],
) -> Result<Vec<u16>, LineageAssayResponseCalibrationError> {
    let expected_cells = first_outcomes.len().saturating_mul(second_outcomes.len());
    let table = model.predict(contrast);
    if table.len() != expected_cells {
        return Err(LineageAssayResponseCalibrationError::InvalidOutput(
            "joint model category count differs from the declared assay outcome product".into(),
        ));
    }
    project_joint_probabilities(&table, first_marginals, second_marginals)
}

fn leave_one_unit_out_joint_brier(
    observations: &[LineageAssayJointCalibrationObservation],
    first_outcomes: &[String],
    second_outcomes: &[String],
    joint_outcomes: &[String],
    maximum_bins: u16,
) -> Result<(u16, u64, u64), LineageAssayResponseCalibrationError> {
    let rows = observations
        .iter()
        .map(|observation| LineageAssayCalibrationObservation {
            experimental_unit_id: observation.experimental_unit_id.clone(),
            treatment_minus_control_coefficient_ppm: observation
                .treatment_minus_control_coefficient_ppm,
            outcome_id: pair_outcome_key(
                &observation.first_outcome_id,
                &observation.second_outcome_id,
            ),
        })
        .collect::<Vec<_>>();
    let mut joint_score_sum = 0_u64;
    let mut independence_score_sum = 0_u64;
    for held_out_index in 0..observations.len() {
        let training = observations
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != held_out_index)
            .map(|(_, observation)| observation)
            .collect::<Vec<_>>();
        let joint_training = rows
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != held_out_index)
            .map(|(_, observation)| observation.clone())
            .collect::<Vec<_>>();
        let first_training = training
            .iter()
            .map(|observation| LineageAssayCalibrationObservation {
                experimental_unit_id: observation.experimental_unit_id.clone(),
                treatment_minus_control_coefficient_ppm: observation
                    .treatment_minus_control_coefficient_ppm,
                outcome_id: observation.first_outcome_id.clone(),
            })
            .collect::<Vec<_>>();
        let second_training = training
            .iter()
            .map(|observation| LineageAssayCalibrationObservation {
                experimental_unit_id: observation.experimental_unit_id.clone(),
                treatment_minus_control_coefficient_ppm: observation
                    .treatment_minus_control_coefficient_ppm,
                outcome_id: observation.second_outcome_id.clone(),
            })
            .collect::<Vec<_>>();
        let joint_model = BinnedOutcomeModel::fit(&joint_training, joint_outcomes, maximum_bins);
        let first_model = BinnedOutcomeModel::fit(&first_training, first_outcomes, maximum_bins);
        let second_model = BinnedOutcomeModel::fit(&second_training, second_outcomes, maximum_bins);
        let held_out = &observations[held_out_index];
        let contrast = held_out.treatment_minus_control_coefficient_ppm;
        let first_marginals = first_model.predict(contrast);
        let second_marginals = second_model.predict(contrast);
        let joint_prediction = projected_joint_prediction(
            &joint_model,
            contrast,
            first_outcomes,
            second_outcomes,
            &first_marginals,
            &second_marginals,
        )?;
        let independence_prediction =
            independent_joint_probabilities(&first_marginals, &second_marginals)?;
        let observed = pair_outcome_key(&held_out.first_outcome_id, &held_out.second_outcome_id);
        let observed_index = joint_outcomes
            .iter()
            .position(|outcome| outcome == &observed)
            .ok_or_else(|| {
                LineageAssayResponseCalibrationError::InvalidInput(
                    "paired calibration row maps to an undeclared joint outcome".into(),
                )
            })?;
        joint_score_sum += brier_million(&joint_prediction, observed_index);
        independence_score_sum += brier_million(&independence_prediction, observed_index);
    }
    let count = observations.len() as u64;
    Ok((
        observations.len() as u16,
        (joint_score_sum + count / 2) / count,
        (independence_score_sum + count / 2) / count,
    ))
}

/// Learn whether two glioma assays share target-dependent outcome noise from paired independent
/// preclinical units. The joint model is evaluated against a held-out product-of-marginals
/// baseline, preserves those calibrated marginal likelihoods exactly, and uses conditional
/// independence outside the observed contrast range or when dependence fails the skill gate.
pub fn calibrate_glioma_lineage_joint_assay_response_model(
    analysis: &LineagePropagationAnalysis,
    candidate_a: &StratifiedAssayCandidate,
    candidate_b: &StratifiedAssayCandidate,
    request: &LineageAssayJointResponseCalibrationRequest,
    paired_observations: &[LineageAssayJointCalibrationObservation],
) -> Result<LineageAssayJointResponseCalibrationRun, LineageAssayResponseCalibrationError> {
    let (first_candidate, second_candidate, mut observations) =
        if candidate_a.action.action_id < candidate_b.action.action_id {
            (candidate_a, candidate_b, paired_observations.to_vec())
        } else {
            (
                candidate_b,
                candidate_a,
                paired_observations
                    .iter()
                    .map(|observation| LineageAssayJointCalibrationObservation {
                        experimental_unit_id: observation.experimental_unit_id.clone(),
                        treatment_minus_control_coefficient_ppm: observation
                            .treatment_minus_control_coefficient_ppm,
                        first_outcome_id: observation.second_outcome_id.clone(),
                        second_outcome_id: observation.first_outcome_id.clone(),
                    })
                    .collect(),
            )
        };
    let first_action_id = first_candidate.action.action_id.as_str();
    let second_action_id = second_candidate.action.action_id.as_str();
    let mut first_outcomes = outcome_ids(first_candidate)?;
    let mut second_outcomes = outcome_ids(second_candidate)?;
    first_outcomes.sort();
    second_outcomes.sort();
    if first_action_id == second_action_id
        || first_candidate.stratum_id != second_candidate.stratum_id
        || first_candidate.action.max_replicates != 1
        || second_candidate.action.max_replicates != 1
    {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "joint calibration requires two distinct, single-use assays for the same glioma source state".into(),
        ));
    }
    if analysis.disposition != LineagePropagationDisposition::Qualified {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "joint calibration requires a qualified P10 lineage-propagation analysis".into(),
        ));
    }
    analysis
        .validate()
        .map_err(|error| LineageAssayResponseCalibrationError::InvalidRequest(error.to_string()))?;
    if !analysis.state_order.contains(&first_candidate.stratum_id)
        || !analysis.state_order.contains(&request.destination_state_id)
        || request.destination_state_id.trim().is_empty()
        || request.model_version.trim().is_empty()
        || request.maximum_quantile_bins == 0
        || request.maximum_quantile_bins > MAX_QUANTILE_BINS
        || request.minimum_marginal_brier_skill_ppm > 1_000_000
        || request.minimum_dependence_brier_skill_ppm > 1_000_000
    {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "joint calibration destination, model version, bin count, or skill threshold is invalid".into(),
        ));
    }
    if !(12..=MAX_CALIBRATION_UNITS).contains(&observations.len()) {
        return Err(LineageAssayResponseCalibrationError::InsufficientEvidence(
            format!(
                "12..={MAX_CALIBRATION_UNITS} paired independent experimental units are required"
            ),
        ));
    }
    let joint_outcome_ids = first_outcomes
        .iter()
        .flat_map(|first| {
            second_outcomes
                .iter()
                .map(move |second| pair_outcome_key(first, second))
        })
        .collect::<Vec<_>>();
    let unit_count = observations.len() as u128;
    let joint_category_count = joint_outcome_ids.len() as u128;
    let projection_work_per_row = 256_u128.saturating_mul(joint_category_count);
    let work = unit_count
        .saturating_mul(unit_count)
        .saturating_mul(
            joint_category_count.saturating_add(u128::from(request.maximum_quantile_bins)),
        )
        .saturating_add(
            unit_count
                .saturating_add(analysis.bootstrap_draws.len() as u128)
                .saturating_mul(projection_work_per_row),
        );
    if work > MAX_JOINT_CALIBRATION_WORK {
        return Err(LineageAssayResponseCalibrationError::InvalidRequest(
            "paired cross-validation exceeds the bounded joint-calibration work budget".into(),
        ));
    }
    let first_declared = first_outcomes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let second_declared = second_outcomes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut units = BTreeSet::new();
    let mut joint_counts = joint_outcome_ids
        .iter()
        .map(|outcome| (outcome.as_str(), 0_u32))
        .collect::<BTreeMap<_, _>>();
    for observation in &observations {
        if observation.experimental_unit_id.trim().is_empty()
            || observation.treatment_minus_control_coefficient_ppm.abs() > MAX_ABS_CONTRAST_PPM
            || !first_declared.contains(observation.first_outcome_id.as_str())
            || !second_declared.contains(observation.second_outcome_id.as_str())
            || !units.insert(observation.experimental_unit_id.as_str())
        {
            return Err(LineageAssayResponseCalibrationError::InvalidInput(
                "paired rows must be unique independent units with an in-range contrast and declared outcomes for both assays".into(),
            ));
        }
        let key = pair_outcome_key(
            &observation.first_outcome_id,
            &observation.second_outcome_id,
        );
        *joint_counts
            .get_mut(key.as_str())
            .expect("paired outcomes were checked against the declared Cartesian product") += 1;
    }
    observations.sort_by(|left, right| left.experimental_unit_id.cmp(&right.experimental_unit_id));
    let minimum_units_per_joint_outcome = joint_counts.values().copied().min().unwrap_or(0);
    let single_request = LineageAssayResponseCalibrationRequest {
        destination_state_id: request.destination_state_id.clone(),
        model_version: request.model_version.clone(),
        maximum_quantile_bins: request.maximum_quantile_bins,
        minimum_brier_skill_ppm: request.minimum_marginal_brier_skill_ppm,
    };
    let first_rows = observations
        .iter()
        .map(|observation| LineageAssayCalibrationObservation {
            experimental_unit_id: observation.experimental_unit_id.clone(),
            treatment_minus_control_coefficient_ppm: observation
                .treatment_minus_control_coefficient_ppm,
            outcome_id: observation.first_outcome_id.clone(),
        })
        .collect::<Vec<_>>();
    let second_rows = observations
        .iter()
        .map(|observation| LineageAssayCalibrationObservation {
            experimental_unit_id: observation.experimental_unit_id.clone(),
            treatment_minus_control_coefficient_ppm: observation
                .treatment_minus_control_coefficient_ppm,
            outcome_id: observation.second_outcome_id.clone(),
        })
        .collect::<Vec<_>>();
    let first_run = calibrate_glioma_lineage_assay_response_model(
        analysis,
        first_candidate,
        &single_request,
        &first_rows,
    )?;
    let second_run = calibrate_glioma_lineage_assay_response_model(
        analysis,
        second_candidate,
        &single_request,
        &second_rows,
    )?;
    let (fold_count, joint_brier, independence_brier) = leave_one_unit_out_joint_brier(
        &observations,
        &first_outcomes,
        &second_outcomes,
        &joint_outcome_ids,
        request.maximum_quantile_bins,
    )?;
    let skill = brier_skill_ppm(joint_brier, independence_brier);
    let disposition = if skill > 0 && skill >= i64::from(request.minimum_dependence_brier_skill_ppm)
    {
        LineageAssayJointResponseCalibrationDisposition::ConditionalDependenceSupported
    } else {
        LineageAssayJointResponseCalibrationDisposition::ConditionalIndependenceBaseline
    };
    let first_model = first_run.response_model;
    let second_model = second_run.response_model;
    let full_rows = observations
        .iter()
        .map(|observation| LineageAssayCalibrationObservation {
            experimental_unit_id: observation.experimental_unit_id.clone(),
            treatment_minus_control_coefficient_ppm: observation
                .treatment_minus_control_coefficient_ppm,
            outcome_id: pair_outcome_key(
                &observation.first_outcome_id,
                &observation.second_outcome_id,
            ),
        })
        .collect::<Vec<_>>();
    let full_joint_model = BinnedOutcomeModel::fit(
        &full_rows,
        &joint_outcome_ids,
        request.maximum_quantile_bins,
    );
    let minimum = observations
        .iter()
        .map(|observation| observation.treatment_minus_control_coefficient_ppm)
        .min()
        .expect("nonempty paired rows were checked");
    let maximum = observations
        .iter()
        .map(|observation| observation.treatment_minus_control_coefficient_ppm)
        .max()
        .expect("nonempty paired rows were checked");
    let mut likelihoods = joint_outcome_ids
        .iter()
        .map(|outcome| {
            let mut parts = outcome.splitn(2, ':');
            let first_length = parts
                .next()
                .expect("encoded pair has first length")
                .parse::<usize>()
                .expect("internal pair length is numeric");
            let remainder = parts.next().expect("encoded pair has first value");
            let (first_outcome_id, second_encoded) = remainder.split_at(first_length);
            let second_length_end = second_encoded
                .find(':')
                .expect("encoded pair has second length");
            let second_length = second_encoded[..second_length_end]
                .parse::<usize>()
                .expect("internal pair length is numeric");
            let second_outcome_id = &second_encoded[second_length_end + 1..];
            debug_assert_eq!(second_outcome_id.len(), second_length);
            LineagePropagationJointOutcomeLikelihood {
                first_outcome_id: first_outcome_id.into(),
                second_outcome_id: second_outcome_id.into(),
                likelihood_milli_by_bootstrap_draw: Vec::with_capacity(
                    analysis.bootstrap_draws.len(),
                ),
            }
        })
        .collect::<Vec<_>>();
    let mut out_of_support = 0_u32;
    let source = analysis
        .state_order
        .iter()
        .position(|state| state == &first_candidate.stratum_id)
        .expect("validated source state exists");
    let destination = analysis
        .state_order
        .iter()
        .position(|state| state == &request.destination_state_id)
        .expect("validated destination state exists");
    let state_count = analysis.state_order.len();
    for (draw_index, draw) in analysis.bootstrap_draws.iter().enumerate() {
        let flat_index = destination * state_count + source;
        let contrast = draw.treatment_coefficients_ppm[flat_index] as i64
            - draw.control_coefficients_ppm[flat_index] as i64;
        let first_marginals = first_outcomes
            .iter()
            .map(|outcome| {
                first_model
                    .outcomes
                    .iter()
                    .find(|candidate| &candidate.outcome_id == outcome)
                    .expect("single model covers every declared outcome")
                    .likelihood_milli_by_bootstrap_draw[draw_index]
            })
            .collect::<Vec<_>>();
        let second_marginals = second_outcomes
            .iter()
            .map(|outcome| {
                second_model
                    .outcomes
                    .iter()
                    .find(|candidate| &candidate.outcome_id == outcome)
                    .expect("single model covers every declared outcome")
                    .likelihood_milli_by_bootstrap_draw[draw_index]
            })
            .collect::<Vec<_>>();
        let probabilities = if disposition
            == LineageAssayJointResponseCalibrationDisposition::ConditionalDependenceSupported
            && (minimum..=maximum).contains(&contrast)
        {
            projected_joint_prediction(
                &full_joint_model,
                contrast,
                &first_outcomes,
                &second_outcomes,
                &first_marginals,
                &second_marginals,
            )?
        } else {
            if contrast < minimum || contrast > maximum {
                out_of_support += 1;
            }
            independent_joint_probabilities(&first_marginals, &second_marginals)?
        };
        for (outcome, probability) in likelihoods.iter_mut().zip(probabilities) {
            outcome.likelihood_milli_by_bootstrap_draw.push(probability);
        }
    }
    let calibration_source_digest = ContentHash::of_value(&joint_calibration_digest_input(
        request,
        first_candidate,
        second_candidate,
        &observations,
    ))
    .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
    let selected_bins = if disposition
        == LineageAssayJointResponseCalibrationDisposition::ConditionalDependenceSupported
    {
        full_joint_model.outcome_counts_by_bin.len()
    } else {
        1
    } as u16;
    let joint_response_model = LineagePropagationJointAssayResponseModel {
        first_action_id: first_action_id.into(),
        second_action_id: second_action_id.into(),
        analysis_digest: analysis.digest.clone(),
        model_version: request.model_version.clone(),
        calibration_source_digest,
        calibration_unit_count: observations.len() as u32,
        outcomes: likelihoods,
    };
    let diagnostics = LineageAssayJointResponseCalibrationDiagnostics {
        calibration_unit_count: observations.len() as u32,
        joint_outcome_count: joint_outcome_ids.len() as u16,
        minimum_units_per_joint_outcome,
        cross_validation_fold_count: fold_count,
        selected_quantile_bin_count: selected_bins,
        leave_one_unit_out_brier_million: joint_brier,
        independence_baseline_brier_million: independence_brier,
        brier_skill_ppm: skill,
        out_of_calibration_support_draw_count: out_of_support,
        calibration_contrast_min_ppm: minimum,
        calibration_contrast_max_ppm: maximum,
    };
    let mut run = LineageAssayJointResponseCalibrationRun {
        feature_id: FEATURE_ID.into(),
        output_schema: JOINT_OUTPUT_SCHEMA.into(),
        first_action_id: first_action_id.into(),
        second_action_id: second_action_id.into(),
        source_state_id: first_candidate.stratum_id.clone(),
        destination_state_id: request.destination_state_id.clone(),
        analysis_digest: analysis.digest.clone(),
        disposition,
        first_response_model: first_model,
        second_response_model: second_model,
        joint_response_model,
        diagnostics,
        digest: ContentHash::of_bytes(b"pending-glioma-joint-assay-response-calibration"),
    };
    run.digest = ContentHash::of_value(&joint_run_digest_input(&run))
        .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
    run.validate(analysis, candidate_a, candidate_b)?;
    Ok(run)
}

impl LineageAssayJointResponseCalibrationRun {
    pub fn validate(
        &self,
        analysis: &LineagePropagationAnalysis,
        candidate_a: &StratifiedAssayCandidate,
        candidate_b: &StratifiedAssayCandidate,
    ) -> Result<(), LineageAssayResponseCalibrationError> {
        let (first, second) = if candidate_a.action.action_id < candidate_b.action.action_id {
            (candidate_a, candidate_b)
        } else {
            (candidate_b, candidate_a)
        };
        let first_outcomes = outcome_ids(first)?.into_iter().collect::<BTreeSet<_>>();
        let second_outcomes = outcome_ids(second)?.into_iter().collect::<BTreeSet<_>>();
        let expected_pairs = first_outcomes
            .iter()
            .flat_map(|first| {
                second_outcomes
                    .iter()
                    .map(move |second| (first.as_str(), second.as_str()))
            })
            .collect::<BTreeSet<_>>();
        let actual_pairs = self
            .joint_response_model
            .outcomes
            .iter()
            .map(|outcome| {
                (
                    outcome.first_outcome_id.as_str(),
                    outcome.second_outcome_id.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != JOINT_OUTPUT_SCHEMA
            || self.first_action_id != first.action.action_id
            || self.second_action_id != second.action.action_id
            || self.source_state_id != first.stratum_id
            || self.source_state_id != second.stratum_id
            || self.analysis_digest != analysis.digest
            || self.joint_response_model.analysis_digest != analysis.digest
            || self.joint_response_model.first_action_id != self.first_action_id
            || self.joint_response_model.second_action_id != self.second_action_id
            || self.first_response_model.action_id != self.first_action_id
            || self.second_response_model.action_id != self.second_action_id
            || self.first_response_model.analysis_digest != analysis.digest
            || self.second_response_model.analysis_digest != analysis.digest
            || self.destination_state_id.trim().is_empty()
            || self.joint_response_model.model_version.trim().is_empty()
            || self.diagnostics.calibration_unit_count < 12
            || self.joint_response_model.calibration_unit_count
                != self.diagnostics.calibration_unit_count
            || actual_pairs != expected_pairs
            || self.joint_response_model.outcomes.iter().any(|outcome| {
                outcome.likelihood_milli_by_bootstrap_draw.len() != analysis.bootstrap_draws.len()
            })
            || (0..analysis.bootstrap_draws.len()).any(|draw| {
                self.joint_response_model
                    .outcomes
                    .iter()
                    .map(|outcome| u32::from(outcome.likelihood_milli_by_bootstrap_draw[draw]))
                    .sum::<u32>()
                    != 1_000
                    || first_outcomes.iter().any(|expected| {
                        let marginal = self
                            .joint_response_model
                            .outcomes
                            .iter()
                            .filter(|outcome| &outcome.first_outcome_id == expected)
                            .map(|outcome| {
                                u32::from(outcome.likelihood_milli_by_bootstrap_draw[draw])
                            })
                            .sum::<u32>();
                        let single = self
                            .first_response_model
                            .outcomes
                            .iter()
                            .find(|outcome| &outcome.outcome_id == expected)
                            .map(|outcome| {
                                u32::from(outcome.likelihood_milli_by_bootstrap_draw[draw])
                            });
                        single != Some(marginal)
                    })
                    || second_outcomes.iter().any(|expected| {
                        let marginal = self
                            .joint_response_model
                            .outcomes
                            .iter()
                            .filter(|outcome| &outcome.second_outcome_id == expected)
                            .map(|outcome| {
                                u32::from(outcome.likelihood_milli_by_bootstrap_draw[draw])
                            })
                            .sum::<u32>();
                        let single = self
                            .second_response_model
                            .outcomes
                            .iter()
                            .find(|outcome| &outcome.outcome_id == expected)
                            .map(|outcome| {
                                u32::from(outcome.likelihood_milli_by_bootstrap_draw[draw])
                            });
                        single != Some(marginal)
                    })
            })
        {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "joint calibration identity, categorical support, normalized probabilities, or exact single-assay marginals are inconsistent".into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&joint_run_digest_input(self))
            .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
        if expected_digest != self.digest {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "joint response calibration digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

fn prevalence_probabilities(
    training: &[LineageAssayCalibrationObservation],
    outcomes: &[String],
) -> Vec<u16> {
    probability_milli(&outcome_counts(training, outcomes))
}

fn brier_million(probabilities: &[u16], observed_index: usize) -> u64 {
    probabilities
        .iter()
        .enumerate()
        .map(|(index, probability)| {
            let expected = if index == observed_index {
                1_000_i64
            } else {
                0_i64
            };
            (i64::from(*probability) - expected).pow(2) as u64
        })
        .sum()
}

fn leave_one_unit_out_brier(
    observations: &[LineageAssayCalibrationObservation],
    outcomes: &[String],
    maximum_bins: u16,
) -> (u16, u64, u64) {
    let mut model_score_sum = 0_u64;
    let mut baseline_score_sum = 0_u64;
    for held_out_index in 0..observations.len() {
        let training = observations
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != held_out_index)
            .map(|(_, observation)| observation.clone())
            .collect::<Vec<_>>();
        let baseline = prevalence_probabilities(&training, outcomes);
        let model = BinnedOutcomeModel::fit(&training, outcomes, maximum_bins);
        let observation = &observations[held_out_index];
        let observed_index = outcomes
            .iter()
            .position(|outcome| outcome == &observation.outcome_id)
            .expect("outcome was validated");
        let local = model.predict(observation.treatment_minus_control_coefficient_ppm);
        model_score_sum += brier_million(&local, observed_index);
        baseline_score_sum += brier_million(&baseline, observed_index);
    }
    let count = observations.len() as u64;
    (
        observations.len() as u16,
        (model_score_sum + count / 2) / count,
        (baseline_score_sum + count / 2) / count,
    )
}

fn brier_skill_ppm(model: u64, baseline: u64) -> i64 {
    if baseline == 0 {
        return 0;
    }
    (((baseline as i128 - model as i128) * 1_000_000) / baseline as i128)
        .clamp(-1_000_000, 1_000_000) as i64
}

/// Calibrate categorical assay likelihoods from independent historical preclinical units.
///
/// Each input contrast must be the paired experimental-unit treatment-minus-control coefficient
/// for `candidate.stratum_id → request.destination_state_id`. The learned likelihoods are
/// evaluated on the corresponding contrasts in each current P10 bootstrap draw. Cross-validation
/// is stratified by outcome and holds out whole experimental units; it never splits cells or
/// barcodes across train/test folds.
pub fn calibrate_glioma_lineage_assay_response_model(
    analysis: &LineagePropagationAnalysis,
    candidate: &StratifiedAssayCandidate,
    request: &LineageAssayResponseCalibrationRequest,
    observations: &[LineageAssayCalibrationObservation],
) -> Result<LineageAssayResponseCalibrationRun, LineageAssayResponseCalibrationError> {
    let outcomes = validate_inputs(analysis, candidate, request, observations)?;
    let mut observations = observations.to_vec();
    observations.sort_by(|left, right| left.experimental_unit_id.cmp(&right.experimental_unit_id));
    let source_index = analysis
        .state_order
        .iter()
        .position(|state| state == &candidate.stratum_id)
        .expect("validated source state exists");
    let destination_index = analysis
        .state_order
        .iter()
        .position(|state| state == &request.destination_state_id)
        .expect("validated destination state exists");
    let state_count = analysis.state_order.len();
    let calibration_source_digest =
        ContentHash::of_value(&calibration_digest_input(request, candidate, &observations))
            .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
    let (fold_count, model_brier, baseline_brier) =
        leave_one_unit_out_brier(&observations, &outcomes, request.maximum_quantile_bins);
    let skill = brier_skill_ppm(model_brier, baseline_brier);
    let disposition = if skill > 0 && skill >= i64::from(request.minimum_brier_skill_ppm) {
        LineageAssayResponseCalibrationDisposition::Predictive
    } else {
        LineageAssayResponseCalibrationDisposition::BaselineOnly
    };
    let baseline = prevalence_probabilities(&observations, &outcomes);
    let mut all_draw_contrasts = Vec::with_capacity(analysis.bootstrap_draws.len());
    for draw in &analysis.bootstrap_draws {
        let flat_index = destination_index * state_count + source_index;
        all_draw_contrasts.push(
            draw.treatment_coefficients_ppm[flat_index] as i64
                - draw.control_coefficients_ppm[flat_index] as i64,
        );
    }
    let minimum = observations
        .iter()
        .map(|observation| observation.treatment_minus_control_coefficient_ppm)
        .min()
        .expect("validated calibration sample is nonempty");
    let maximum = observations
        .iter()
        .map(|observation| observation.treatment_minus_control_coefficient_ppm)
        .max()
        .expect("validated calibration sample is nonempty");
    let full_model = if disposition == LineageAssayResponseCalibrationDisposition::Predictive {
        Some(BinnedOutcomeModel::fit(
            &observations,
            &outcomes,
            request.maximum_quantile_bins,
        ))
    } else {
        None
    };
    let mut likelihoods = outcomes
        .iter()
        .map(|outcome_id| LineagePropagationOutcomeLikelihood {
            outcome_id: outcome_id.clone(),
            likelihood_milli_by_bootstrap_draw: Vec::with_capacity(all_draw_contrasts.len()),
        })
        .collect::<Vec<_>>();
    let mut out_of_support = 0_u32;
    for contrast in &all_draw_contrasts {
        let probabilities = if let Some(model) = &full_model {
            if *contrast < minimum || *contrast > maximum {
                out_of_support += 1;
                baseline.clone()
            } else {
                model.predict(*contrast)
            }
        } else {
            baseline.clone()
        };
        for (outcome, probability) in likelihoods.iter_mut().zip(probabilities) {
            outcome.likelihood_milli_by_bootstrap_draw.push(probability);
        }
    }
    let selected_bins = full_model
        .as_ref()
        .map_or(1, |model| model.outcome_counts_by_bin.len()) as u16;
    let unit_counts = outcome_counts(&observations, &outcomes);
    let diagnostics = LineageAssayResponseCalibrationDiagnostics {
        calibration_unit_count: observations.len() as u32,
        outcome_count: outcomes.len() as u16,
        minimum_units_per_outcome: unit_counts.iter().copied().min().unwrap_or(0),
        cross_validation_fold_count: fold_count,
        selected_quantile_bin_count: selected_bins,
        leave_one_unit_out_brier_million: model_brier,
        prevalence_baseline_brier_million: baseline_brier,
        brier_skill_ppm: skill,
        out_of_calibration_support_draw_count: out_of_support,
        calibration_contrast_min_ppm: minimum,
        calibration_contrast_max_ppm: maximum,
    };
    let response_model = LineagePropagationAssayResponseModel {
        action_id: candidate.action.action_id.clone(),
        analysis_digest: analysis.digest.clone(),
        model_version: request.model_version.clone(),
        calibration_source_digest,
        calibration_unit_count: observations.len() as u32,
        outcomes: likelihoods,
    };
    let mut run = LineageAssayResponseCalibrationRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        action_id: candidate.action.action_id.clone(),
        source_state_id: candidate.stratum_id.clone(),
        destination_state_id: request.destination_state_id.clone(),
        analysis_digest: analysis.digest.clone(),
        disposition,
        response_model,
        diagnostics,
        digest: ContentHash::of_bytes(b"pending-glioma-lineage-assay-response-calibration"),
    };
    run.digest = ContentHash::of_value(&run_digest_input(&run))
        .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
    run.validate(analysis, candidate)?;
    Ok(run)
}

impl LineageAssayResponseCalibrationRun {
    pub fn validate(
        &self,
        analysis: &LineagePropagationAnalysis,
        candidate: &StratifiedAssayCandidate,
    ) -> Result<(), LineageAssayResponseCalibrationError> {
        let expected_outcomes = candidate
            .action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        let actual_outcomes = self
            .response_model
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.action_id != candidate.action.action_id
            || self.source_state_id != candidate.stratum_id
            || self.analysis_digest != analysis.digest
            || self.response_model.analysis_digest != analysis.digest
            || self.response_model.action_id != candidate.action.action_id
            || self.destination_state_id.is_empty()
            || !analysis.state_order.contains(&self.destination_state_id)
            || self.response_model.model_version.trim().is_empty()
            || self.response_model.calibration_unit_count != self.diagnostics.calibration_unit_count
            || self.diagnostics.calibration_unit_count < 12
            || self.diagnostics.outcome_count as usize != expected_outcomes.len()
            || expected_outcomes != actual_outcomes
            || self.response_model.outcomes.iter().any(|outcome| {
                outcome.likelihood_milli_by_bootstrap_draw.len() != analysis.bootstrap_draws.len()
                    || outcome
                        .likelihood_milli_by_bootstrap_draw
                        .iter()
                        .any(|probability| u64::from(*probability) > PROBABILITY_MILLI)
            })
            || (0..analysis.bootstrap_draws.len()).any(|draw_index| {
                self.response_model
                    .outcomes
                    .iter()
                    .map(|outcome| {
                        u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index])
                    })
                    .sum::<u64>()
                    != PROBABILITY_MILLI
            })
            || (self.disposition == LineageAssayResponseCalibrationDisposition::BaselineOnly
                && (0..analysis.bootstrap_draws.len()).any(|draw_index| {
                    self.response_model.outcomes.iter().any(|outcome| {
                        outcome.likelihood_milli_by_bootstrap_draw[draw_index]
                            != outcome.likelihood_milli_by_bootstrap_draw[0]
                    })
                }))
        {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "identity, analysis binding, calibration support, or per-draw categorical likelihoods are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&run_digest_input(self))
            .map_err(|error| LineageAssayResponseCalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LineageAssayResponseCalibrationError::InvalidOutput(
                "response calibration digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod joint_projection_tests {
    use super::*;

    #[test]
    fn controlled_rounding_preserves_sparse_exact_marginals() {
        let table = project_joint_probabilities(&[996, 1, 1, 2], &[1, 999], &[500, 500]).unwrap();
        assert_eq!(
            table.iter().map(|value| u32::from(*value)).sum::<u32>(),
            1_000
        );
        assert_eq!(u32::from(table[0]) + u32::from(table[1]), 1);
        assert_eq!(u32::from(table[2]) + u32::from(table[3]), 999);
        assert_eq!(u32::from(table[0]) + u32::from(table[2]), 500);
        assert_eq!(u32::from(table[1]) + u32::from(table[3]), 500);
    }

    #[test]
    fn independent_coupling_keeps_rare_outcome_marginals_exact() {
        let rows = [1, 249, 750];
        let columns = [1, 499, 500];
        let table = independent_joint_probabilities(&rows, &columns).unwrap();
        for (row, expected) in rows.iter().enumerate() {
            assert_eq!(
                table[row * columns.len()..(row + 1) * columns.len()]
                    .iter()
                    .map(|value| u32::from(*value))
                    .sum::<u32>(),
                u32::from(*expected)
            );
        }
        for (column, expected) in columns.iter().enumerate() {
            assert_eq!(
                (0..rows.len())
                    .map(|row| u32::from(table[row * columns.len() + column]))
                    .sum::<u32>(),
                u32::from(*expected)
            );
        }
    }
}
