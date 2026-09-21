//! Prospective long-horizon calibration and drift control for glioma evidence surveillance.
//!
//! P01-F11 calibrates one source-family surface. This feature extends that contract across
//! explicit temporal windows: it compares quality-weighted predictions with later resolved
//! outcomes, measures calibration error and reliability drift, requires independent-group
//! coverage, and routes degrading families to review or reacquisition. Unknown, stale, negative,
//! and contradictory outcomes remain visible. The feature never interprets prose, infers a
//! mechanism, or turns a calibration score into a clinical decision.

use crate::glioma::evidence::EvidenceState;
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaLongHorizonCalibration1@1";
pub const MAX_OBSERVATIONS: usize = 100_000;
pub const MAX_FAMILIES: usize = 4_096;
pub const MAX_WINDOWS: usize = 64;
pub const MAX_WINDOW_ROWS: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationObservation {
    pub observation_id: String,
    pub source_family: String,
    pub epoch: u32,
    pub predicted_support_milli: u16,
    pub outcome: EvidenceState,
    pub quality_milli: u16,
    pub independent_group: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationRequest {
    pub objective: String,
    pub current_epoch: u32,
    pub window_count: usize,
    pub window_size_epochs: u32,
    pub minimum_observations_per_window: usize,
    pub minimum_resolved_per_window: usize,
    pub minimum_independent_groups_per_window: usize,
    pub minimum_qualified_windows: usize,
    pub maximum_expected_calibration_error_milli: u16,
    pub maximum_ece_delta_milli: u16,
    pub maximum_reliability_drop_milli: u16,
    pub observations: Vec<LongHorizonCalibrationObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationOmission {
    pub observation_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongHorizonWindowDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationWindow {
    pub window_id: String,
    pub source_family: String,
    pub window_index: usize,
    pub lower_epoch: u32,
    pub upper_epoch: u32,
    pub observation_order: Vec<String>,
    pub resolved_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub resolved_count: usize,
    pub unknown_count: usize,
    pub independent_group_count: usize,
    pub predicted_support_milli: u16,
    pub observed_support_milli: u16,
    pub expected_calibration_error_milli: u16,
    pub brier_milli: u16,
    pub reliability_milli: u16,
    pub disposition: LongHorizonWindowDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongHorizonCalibrationDrift {
    Stable,
    Improving,
    Degrading,
    Volatile,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongHorizonCalibrationAction {
    ContinueSurveillance,
    RecalibrateFamily,
    ReviewEvidence,
    AcquireCoverage,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationFamily {
    pub source_family: String,
    pub window_order: Vec<String>,
    pub observed_window_count: usize,
    pub qualified_window_count: usize,
    pub baseline_window_index: Option<usize>,
    pub recent_window_index: Option<usize>,
    pub baseline_expected_calibration_error_milli: u16,
    pub recent_expected_calibration_error_milli: u16,
    pub expected_calibration_error_delta_milli: i32,
    pub baseline_reliability_milli: u16,
    pub recent_reliability_milli: u16,
    pub reliability_delta_milli: i32,
    pub drift: LongHorizonCalibrationDrift,
    pub action: LongHorizonCalibrationAction,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongHorizonCalibrationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongHorizonCalibrationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub current_epoch: u32,
    pub window_count: usize,
    pub window_size_epochs: u32,
    pub observation_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub omissions: Vec<LongHorizonCalibrationOmission>,
    pub source_family_order: Vec<String>,
    pub windows: Vec<LongHorizonCalibrationWindow>,
    pub families: Vec<LongHorizonCalibrationFamily>,
    pub negative_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub review_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: LongHorizonCalibrationDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LongHorizonCalibrationError {
    #[error("long-horizon calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("long-horizon calibration observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("long-horizon calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("long-horizon calibration digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Default)]
struct WindowAggregate {
    observation_order: Vec<String>,
    resolved_order: Vec<String>,
    negative_order: Vec<String>,
    unknown_order: Vec<String>,
    groups: BTreeSet<String>,
    predicted_weighted_sum: u128,
    quality_sum: u128,
    observed_weighted_sum: u128,
    resolved_quality_sum: u128,
    brier_weighted_sum: u128,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn outcome_value(outcome: EvidenceState) -> Option<u16> {
    match outcome {
        EvidenceState::Supported => Some(1_000),
        EvidenceState::Negative | EvidenceState::Contradicted => Some(0),
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => None,
    }
}

fn window_id(source_family: &str, index: usize) -> String {
    format!("{source_family}::window-{index:03}")
}

fn window_bounds(current_epoch: u32, window_size_epochs: u32, index: usize) -> (u32, u32) {
    let upper = current_epoch.saturating_sub((index as u32).saturating_mul(window_size_epochs));
    let lower = upper.saturating_sub(window_size_epochs.saturating_sub(1));
    (lower, upper)
}

fn digest_input(output: &LongHorizonCalibrationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "current_epoch": output.current_epoch,
        "window_count": output.window_count,
        "window_size_epochs": output.window_size_epochs,
        "observation_order": output.observation_order,
        "selected_order": output.selected_order,
        "omitted_order": output.omitted_order,
        "omissions": output.omissions,
        "source_family_order": output.source_family_order,
        "windows": output.windows,
        "families": output.families,
        "negative_order": output.negative_order,
        "unknown_order": output.unknown_order,
        "review_order": output.review_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(
    request: &LongHorizonCalibrationRequest,
) -> Result<(), LongHorizonCalibrationError> {
    if request.objective.trim().is_empty()
        || request.current_epoch == 0
        || request.window_count < 2
        || request.window_count > MAX_WINDOWS
        || request.window_size_epochs == 0
        || request.minimum_observations_per_window == 0
        || request.minimum_resolved_per_window == 0
        || request.minimum_independent_groups_per_window == 0
        || request.minimum_qualified_windows == 0
        || request.minimum_qualified_windows > request.window_count
        || request.maximum_expected_calibration_error_milli > 1_000
        || request.maximum_ece_delta_milli > 1_000
        || request.maximum_reliability_drop_milli > 1_000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(LongHorizonCalibrationError::InvalidRequest(
            "objective, current epoch, bounded temporal windows, positive floors, and observations are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.source_family.trim().is_empty()
            || observation.epoch == 0
            || observation.epoch > request.current_epoch
            || observation.predicted_support_milli > 1_000
            || observation.quality_milli == 0
            || observation.quality_milli > 1_000
            || observation.independent_group.trim().is_empty()
            || observation.artifact.validate().is_err()
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(LongHorizonCalibrationError::InvalidObservation(
                "observations require unique ids, bounded epochs/scores, independent groups, quality, and valid local artifacts".into(),
            ));
        }
    }
    if request
        .observations
        .iter()
        .map(|observation| observation.source_family.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        > MAX_FAMILIES
    {
        return Err(LongHorizonCalibrationError::InvalidObservation(
            "source-family bound exceeded".into(),
        ));
    }
    Ok(())
}

fn build_window(
    source_family: &str,
    index: usize,
    request: &LongHorizonCalibrationRequest,
    aggregate: WindowAggregate,
) -> LongHorizonCalibrationWindow {
    let total_count = aggregate.observation_order.len();
    let resolved_count = aggregate.resolved_order.len();
    let predicted_support_milli = if aggregate.quality_sum == 0 {
        0
    } else {
        (aggregate.predicted_weighted_sum / aggregate.quality_sum).min(1_000) as u16
    };
    let observed_support_milli = if aggregate.resolved_quality_sum == 0 {
        0
    } else {
        (aggregate.observed_weighted_sum / aggregate.resolved_quality_sum).min(1_000) as u16
    };
    let expected_calibration_error_milli = if resolved_count == 0 {
        1_000
    } else {
        u64::from(predicted_support_milli)
            .abs_diff(u64::from(observed_support_milli))
            .min(1_000) as u16
    };
    let brier_milli = if aggregate.resolved_quality_sum == 0 {
        1_000
    } else {
        (aggregate.brier_weighted_sum / aggregate.resolved_quality_sum).min(1_000) as u16
    };
    let reliability_milli = if total_count == 0 {
        0
    } else {
        u16::from(1_000_u16.saturating_sub(expected_calibration_error_milli))
            .saturating_mul(resolved_count.min(1_000) as u16)
            .saturating_div(total_count.clamp(1, 1_000) as u16)
    };
    let disposition = if total_count >= request.minimum_observations_per_window
        && resolved_count >= request.minimum_resolved_per_window
        && aggregate.groups.len() >= request.minimum_independent_groups_per_window
        && expected_calibration_error_milli <= request.maximum_expected_calibration_error_milli
    {
        LongHorizonWindowDisposition::Qualified
    } else if resolved_count > 0 {
        LongHorizonWindowDisposition::Partial
    } else {
        LongHorizonWindowDisposition::Unresolved
    };
    let (lower_epoch, upper_epoch) =
        window_bounds(request.current_epoch, request.window_size_epochs, index);
    LongHorizonCalibrationWindow {
        window_id: window_id(source_family, index),
        source_family: source_family.into(),
        window_index: index,
        lower_epoch,
        upper_epoch,
        observation_order: aggregate.observation_order,
        resolved_order: aggregate.resolved_order,
        negative_order: aggregate.negative_order,
        unknown_order: aggregate.unknown_order,
        resolved_count,
        unknown_count: total_count.saturating_sub(resolved_count),
        independent_group_count: aggregate.groups.len(),
        predicted_support_milli,
        observed_support_milli,
        expected_calibration_error_milli,
        brier_milli,
        reliability_milli,
        disposition,
    }
}

fn action_for_drift(
    drift: LongHorizonCalibrationDrift,
    qualified_window_count: usize,
    minimum_qualified_windows: usize,
) -> LongHorizonCalibrationAction {
    if qualified_window_count < minimum_qualified_windows {
        return LongHorizonCalibrationAction::AcquireCoverage;
    }
    match drift {
        LongHorizonCalibrationDrift::Stable | LongHorizonCalibrationDrift::Improving => {
            LongHorizonCalibrationAction::ContinueSurveillance
        }
        LongHorizonCalibrationDrift::Degrading => LongHorizonCalibrationAction::RecalibrateFamily,
        LongHorizonCalibrationDrift::Volatile => LongHorizonCalibrationAction::ReviewEvidence,
        LongHorizonCalibrationDrift::Insufficient => LongHorizonCalibrationAction::Hold,
    }
}

impl LongHorizonCalibrationAnalysis {
    pub fn validate(&self) -> Result<(), LongHorizonCalibrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.current_epoch == 0
            || self.window_count < 2
            || self.window_count > MAX_WINDOWS
            || self.window_size_epochs == 0
            || !canonical(&self.observation_order)
            || !unique_nonempty(&self.observation_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.source_family_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.unknown_order)
            || !canonical(&self.review_order)
            || !canonical(&self.uncertainty)
            || self.omissions.len() != self.omitted_order.len()
            || self
                .omissions
                .iter()
                .map(|omission| omission.observation_id.clone())
                .collect::<Vec<_>>()
                != self.omitted_order
            || self.windows.len() > MAX_WINDOW_ROWS
            || self.windows.windows(2).any(|pair| {
                (pair[0].source_family.as_str(), pair[0].window_index)
                    >= (pair[1].source_family.as_str(), pair[1].window_index)
            })
            || self.families.len() != self.source_family_order.len()
            || self
                .families
                .iter()
                .map(|family| family.source_family.clone())
                .collect::<Vec<_>>()
                != self.source_family_order
            || self
                .families
                .windows(2)
                .any(|pair| pair[0].source_family >= pair[1].source_family)
            || self.families.iter().any(|family| {
                family.window_order.len() != self.window_count
                    || family.window_order
                        != self
                            .windows
                            .iter()
                            .filter(|window| window.source_family == family.source_family)
                            .map(|window| window.window_id.clone())
                            .collect::<Vec<_>>()
            })
        {
            return Err(LongHorizonCalibrationError::InvalidOutput(
                "identity, temporal ordering, omission partition, family alignment, or bounds are invalid".into(),
            ));
        }
        let observation_ids = self
            .observation_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let omitted_ids = self.omitted_order.iter().cloned().collect::<BTreeSet<_>>();
        if selected_ids.len() + omitted_ids.len() != observation_ids.len()
            || selected_ids
                .union(&omitted_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != observation_ids
            || self
                .windows
                .iter()
                .flat_map(|window| window.observation_order.iter().cloned())
                .collect::<BTreeSet<_>>()
                != selected_ids
            || self.omissions.iter().any(|omission| {
                omission.observation_id.trim().is_empty() || omission.reason.trim().is_empty()
            })
            || self.windows.iter().any(|window| {
                window.window_id != window_id(&window.source_family, window.window_index)
                    || window.window_index >= self.window_count
                    || window.lower_epoch > window.upper_epoch
                    || window.upper_epoch > self.current_epoch
                    || !canonical(&window.observation_order)
                    || !canonical(&window.resolved_order)
                    || !canonical(&window.negative_order)
                    || !canonical(&window.unknown_order)
                    || window.resolved_count != window.resolved_order.len()
                    || window.unknown_count > window.observation_order.len()
                    || window.independent_group_count > window.observation_order.len()
                    || window.predicted_support_milli > 1_000
                    || window.observed_support_milli > 1_000
                    || window.expected_calibration_error_milli > 1_000
                    || window.brier_milli > 1_000
                    || window.reliability_milli > 1_000
            })
            || self.windows.iter().any(|window| {
                let observation_ids = window
                    .observation_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let resolved_ids = window
                    .resolved_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let unknown_ids = window
                    .unknown_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let negative_ids = window
                    .negative_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                resolved_ids.intersection(&unknown_ids).next().is_some()
                    || resolved_ids.intersection(&negative_ids).next().is_some()
                    || unknown_ids.intersection(&negative_ids).next().is_some()
                    || !resolved_ids.is_subset(&observation_ids)
                    || !unknown_ids.is_subset(&observation_ids)
                    || !negative_ids.is_subset(&observation_ids)
                    || resolved_ids.len() + unknown_ids.len() != observation_ids.len()
                    || window.unknown_count != unknown_ids.len()
            })
            || self.families.iter().any(|family| {
                family.source_family.trim().is_empty()
                    || !canonical(&family.window_order)
                    || family.observed_window_count > self.window_count
                    || family.qualified_window_count > family.observed_window_count
                    || family.baseline_expected_calibration_error_milli > 1_000
                    || family.recent_expected_calibration_error_milli > 1_000
                    || family.baseline_reliability_milli > 1_000
                    || family.recent_reliability_milli > 1_000
                    || !canonical(&family.uncertainty)
            })
        {
            return Err(LongHorizonCalibrationError::InvalidOutput(
                "window observation partition, window bounds, score bounds, or family summary is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LongHorizonCalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LongHorizonCalibrationError::Digest(
                "long-horizon calibration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze source-family calibration over deterministic retrospective windows.
pub fn calibrate_glioma_evidence_long_horizon(
    request: &LongHorizonCalibrationRequest,
) -> Result<LongHorizonCalibrationAnalysis, LongHorizonCalibrationError> {
    validate_request(request)?;
    let mut observation_order = request
        .observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    let mut negative_order = BTreeSet::new();
    let mut unknown_order = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut selected_order = BTreeSet::new();
    let mut omitted = Vec::new();
    let horizon = (request.window_count as u32).saturating_mul(request.window_size_epochs);
    let mut grouped = BTreeMap::<(String, usize), WindowAggregate>::new();
    for observation in &request.observations {
        families.insert(observation.source_family.clone());
        match observation.outcome {
            EvidenceState::Negative | EvidenceState::Contradicted => {
                negative_order.insert(observation.observation_id.clone());
            }
            EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => {
                unknown_order.insert(observation.observation_id.clone());
            }
            EvidenceState::Supported => {}
        }
        let age = request.current_epoch.saturating_sub(observation.epoch);
        if age >= horizon {
            omitted.push(LongHorizonCalibrationOmission {
                observation_id: observation.observation_id.clone(),
                reason: "outside-calibration-horizon".into(),
            });
            continue;
        }
        let index = (age / request.window_size_epochs) as usize;
        let aggregate = grouped
            .entry((observation.source_family.clone(), index))
            .or_default();
        aggregate
            .observation_order
            .push(observation.observation_id.clone());
        aggregate
            .groups
            .insert(observation.independent_group.clone());
        aggregate.quality_sum += u128::from(observation.quality_milli);
        aggregate.predicted_weighted_sum +=
            u128::from(observation.predicted_support_milli) * u128::from(observation.quality_milli);
        selected_order.insert(observation.observation_id.clone());
        if let Some(outcome) = outcome_value(observation.outcome) {
            aggregate
                .resolved_order
                .push(observation.observation_id.clone());
            aggregate.resolved_quality_sum += u128::from(observation.quality_milli);
            aggregate.observed_weighted_sum +=
                u128::from(outcome) * u128::from(observation.quality_milli);
            let error = i64::from(observation.predicted_support_milli) - i64::from(outcome);
            aggregate.brier_weighted_sum += u128::from(error.unsigned_abs())
                * u128::from(error.unsigned_abs())
                * u128::from(observation.quality_milli)
                / 1_000;
            if outcome == 0 {
                aggregate
                    .negative_order
                    .push(observation.observation_id.clone());
            }
        } else {
            aggregate
                .unknown_order
                .push(observation.observation_id.clone());
        }
    }
    omitted.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let source_family_order = families.into_iter().collect::<Vec<_>>();
    let mut windows = Vec::new();
    let mut family_summaries = Vec::new();
    let mut review_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for source_family in &source_family_order {
        let mut family_windows = Vec::new();
        for index in 0..request.window_count {
            let aggregate = grouped
                .remove(&(source_family.clone(), index))
                .unwrap_or_default();
            let mut window = build_window(source_family, index, request, aggregate);
            window.observation_order.sort();
            window.resolved_order.sort();
            window.negative_order.sort();
            window.unknown_order.sort();
            if window.disposition != LongHorizonWindowDisposition::Qualified {
                uncertainty.insert(format!(
                    "{source_family}:window-{index:03}-{:?}",
                    window.disposition
                ));
            }
            family_windows.push(window);
        }
        let observed_windows = family_windows
            .iter()
            .filter(|window| !window.observation_order.is_empty())
            .collect::<Vec<_>>();
        let qualified_window_count = family_windows
            .iter()
            .filter(|window| window.disposition == LongHorizonWindowDisposition::Qualified)
            .count();
        let recent = family_windows
            .iter()
            .find(|window| !window.observation_order.is_empty());
        let baseline = family_windows
            .iter()
            .rev()
            .find(|window| !window.observation_order.is_empty());
        let (
            baseline_index,
            recent_index,
            baseline_ece,
            recent_ece,
            baseline_reliability,
            recent_reliability,
        ) = match (baseline, recent) {
            (Some(baseline), Some(recent)) => (
                Some(baseline.window_index),
                Some(recent.window_index),
                baseline.expected_calibration_error_milli,
                recent.expected_calibration_error_milli,
                baseline.reliability_milli,
                recent.reliability_milli,
            ),
            _ => (None, None, 1_000, 0, 0, 0),
        };
        let ece_delta = i32::from(recent_ece) - i32::from(baseline_ece);
        let reliability_delta = i32::from(recent_reliability) - i32::from(baseline_reliability);
        let ece_range = observed_windows
            .iter()
            .map(|window| window.expected_calibration_error_milli)
            .min()
            .zip(
                observed_windows
                    .iter()
                    .map(|window| window.expected_calibration_error_milli)
                    .max(),
            )
            .map(|(min, max)| max.saturating_sub(min))
            .unwrap_or(1_000);
        let drift = if observed_windows.len() < request.minimum_qualified_windows {
            LongHorizonCalibrationDrift::Insufficient
        } else if ece_range
            > request
                .maximum_ece_delta_milli
                .saturating_mul(2)
                .max(request.maximum_ece_delta_milli)
        {
            LongHorizonCalibrationDrift::Volatile
        } else if ece_delta > i32::from(request.maximum_ece_delta_milli)
            || reliability_delta < -i32::from(request.maximum_reliability_drop_milli)
        {
            LongHorizonCalibrationDrift::Degrading
        } else if ece_delta < -i32::from(request.maximum_ece_delta_milli)
            || reliability_delta > i32::from(request.maximum_reliability_drop_milli)
        {
            LongHorizonCalibrationDrift::Improving
        } else {
            LongHorizonCalibrationDrift::Stable
        };
        let action = action_for_drift(
            drift,
            qualified_window_count,
            request.minimum_qualified_windows,
        );
        if !matches!(action, LongHorizonCalibrationAction::ContinueSurveillance) {
            review_order.insert(source_family.clone());
        }
        let mut family_uncertainty = family_windows
            .iter()
            .filter(|window| window.disposition != LongHorizonWindowDisposition::Qualified)
            .map(|window| format!("{}:window-{}", source_family, window.window_index))
            .collect::<Vec<_>>();
        if drift == LongHorizonCalibrationDrift::Degrading {
            family_uncertainty.push("recent-calibration-degrades-against-baseline".into());
        }
        family_uncertainty.sort();
        family_uncertainty.dedup();
        family_summaries.push(LongHorizonCalibrationFamily {
            source_family: source_family.clone(),
            window_order: family_windows
                .iter()
                .map(|window| window.window_id.clone())
                .collect(),
            observed_window_count: observed_windows.len(),
            qualified_window_count,
            baseline_window_index: baseline_index,
            recent_window_index: recent_index,
            baseline_expected_calibration_error_milli: baseline_ece,
            recent_expected_calibration_error_milli: recent_ece,
            expected_calibration_error_delta_milli: ece_delta,
            baseline_reliability_milli: baseline_reliability,
            recent_reliability_milli: recent_reliability,
            reliability_delta_milli: reliability_delta,
            drift,
            action,
            uncertainty: family_uncertainty,
        });
        windows.extend(family_windows);
    }
    let omitted_order = omitted
        .iter()
        .map(|omission| omission.observation_id.clone())
        .collect::<Vec<_>>();
    let selected_order = selected_order.into_iter().collect::<Vec<_>>();
    let disposition = if family_summaries.is_empty()
        || family_summaries
            .iter()
            .all(|family| family.drift == LongHorizonCalibrationDrift::Insufficient)
    {
        LongHorizonCalibrationDisposition::Unresolved
    } else if family_summaries.iter().any(|family| {
        !matches!(
            family.action,
            LongHorizonCalibrationAction::ContinueSurveillance
        )
    }) {
        LongHorizonCalibrationDisposition::Partial
    } else {
        LongHorizonCalibrationDisposition::Qualified
    };
    let next_route = if family_summaries
        .iter()
        .any(|family| matches!(family.action, LongHorizonCalibrationAction::AcquireCoverage))
    {
        "glioma_federated_evidence_acquisition_policy"
    } else if family_summaries.iter().any(|family| {
        matches!(
            family.action,
            LongHorizonCalibrationAction::RecalibrateFamily
                | LongHorizonCalibrationAction::ReviewEvidence
        )
    }) {
        "glioma_evidence_prospective_triage"
    } else {
        "glioma_evidence_knowledge_bridge"
    };
    let mut output = LongHorizonCalibrationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        current_epoch: request.current_epoch,
        window_count: request.window_count,
        window_size_epochs: request.window_size_epochs,
        observation_order,
        selected_order,
        omitted_order,
        omissions: omitted,
        source_family_order,
        windows,
        families: family_summaries,
        negative_order: negative_order.into_iter().collect(),
        unknown_order: unknown_order.into_iter().collect(),
        review_order: review_order.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-long-horizon-calibration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LongHorizonCalibrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-long-calibration+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn observation(
        id: &str,
        family: &str,
        epoch: u32,
        predicted: u16,
        outcome: EvidenceState,
    ) -> LongHorizonCalibrationObservation {
        LongHorizonCalibrationObservation {
            observation_id: id.into(),
            source_family: family.into(),
            epoch,
            predicted_support_milli: predicted,
            outcome,
            quality_milli: 900,
            independent_group: format!("group-{id}"),
            artifact: artifact(id),
        }
    }

    fn request(
        observations: Vec<LongHorizonCalibrationObservation>,
    ) -> LongHorizonCalibrationRequest {
        LongHorizonCalibrationRequest {
            objective: "calibrate glioma evidence over time".into(),
            current_epoch: 12,
            window_count: 3,
            window_size_epochs: 4,
            minimum_observations_per_window: 2,
            minimum_resolved_per_window: 2,
            minimum_independent_groups_per_window: 2,
            minimum_qualified_windows: 2,
            maximum_expected_calibration_error_milli: 250,
            maximum_ece_delta_milli: 150,
            maximum_reliability_drop_milli: 150,
            observations,
        }
    }

    #[test]
    fn stable_multi_window_family_qualifies_and_routes_to_bridge() {
        let report = calibrate_glioma_evidence_long_horizon(&request(vec![
            observation("recent-a", "literature", 12, 800, EvidenceState::Supported),
            observation("recent-b", "literature", 11, 700, EvidenceState::Supported),
            observation("old-a", "literature", 4, 800, EvidenceState::Supported),
            observation("old-b", "literature", 3, 700, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            report.disposition,
            LongHorizonCalibrationDisposition::Qualified
        );
        assert_eq!(
            report.families[0].drift,
            LongHorizonCalibrationDrift::Stable
        );
        assert_eq!(report.next_route, "glioma_evidence_knowledge_bridge");
        report.validate().unwrap();
    }

    #[test]
    fn degrading_recent_window_routes_recalibration_review() {
        let mut request = request(vec![
            observation("recent-a", "assay", 12, 400, EvidenceState::Negative),
            observation("recent-b", "assay", 11, 400, EvidenceState::Negative),
            observation("old-a", "assay", 4, 800, EvidenceState::Supported),
            observation("old-b", "assay", 3, 800, EvidenceState::Supported),
        ]);
        request.maximum_expected_calibration_error_milli = 500;
        let report = calibrate_glioma_evidence_long_horizon(&request).unwrap();
        assert_eq!(
            report.families[0].drift,
            LongHorizonCalibrationDrift::Degrading
        );
        assert_eq!(
            report.families[0].action,
            LongHorizonCalibrationAction::RecalibrateFamily
        );
        assert_eq!(report.next_route, "glioma_evidence_prospective_triage");
        report.validate().unwrap();
    }

    #[test]
    fn missing_window_coverage_is_explicit_not_interpolated() {
        let report = calibrate_glioma_evidence_long_horizon(&request(vec![
            observation("recent-a", "sparse", 12, 800, EvidenceState::Supported),
            observation("old-a", "sparse", 4, 800, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            report.families[0].action,
            LongHorizonCalibrationAction::AcquireCoverage
        );
        assert_eq!(
            report.next_route,
            "glioma_federated_evidence_acquisition_policy"
        );
        assert!(report
            .uncertainty
            .iter()
            .any(|value| value.contains("window-001")));
        report.validate().unwrap();
    }

    #[test]
    fn outside_horizon_observations_are_omitted_and_replay_is_stable() {
        let mut request = request(vec![
            observation("recent-a", "literature", 16, 800, EvidenceState::Supported),
            observation("recent-b", "literature", 15, 700, EvidenceState::Supported),
            observation("old-a", "literature", 1, 800, EvidenceState::Supported),
        ]);
        request.current_epoch = 16;
        let first = calibrate_glioma_evidence_long_horizon(&request).unwrap();
        request.observations.reverse();
        let second = calibrate_glioma_evidence_long_horizon(&request).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.omitted_order, vec!["old-a"]);
        assert_eq!(first.omissions[0].reason, "outside-calibration-horizon");
    }
}
