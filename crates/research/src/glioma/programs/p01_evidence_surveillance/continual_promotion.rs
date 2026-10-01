//! Continual promotion and rollback control for the federated glioma research commons.
//!
//! P01-F31 schedules many advisory cycles. This P01-F32 feature closes the loop over later, independently
//! observed outcomes: it evaluates rolling windows, requires replay and independent-group quality
//! gates, compares recent performance with a declared baseline, and emits an explicit
//! promote/continue/rollback/hold decision. A negative or contradictory result is never counted as
//! a successful promotion signal, and a missing observation is not interpolated into a pass.

use super::federated_batch_scheduler::FederatedBatchSchedule;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaContinualPromotionControl1@1";
pub const MAX_OBSERVATIONS: usize = 100_000;
pub const MAX_WINDOWS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinualOutcomeState {
    Succeeded,
    Negative,
    Contradicted,
    Unknown,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionWindowDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinualPromotionStatus {
    Promote,
    Continue,
    Rollback,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualPromotionObservation {
    pub observation_id: String,
    pub candidate_id: String,
    pub epoch: u32,
    pub outcome: ContinualOutcomeState,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub independent_groups: usize,
    pub replay_count: usize,
    pub cost_units: u64,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualPromotionRequest {
    pub objective: String,
    pub capability_id: String,
    pub current_epoch: u32,
    pub window_count: usize,
    pub window_size_epochs: u32,
    pub minimum_observations_per_window: usize,
    pub minimum_independent_groups_per_window: usize,
    pub minimum_quality_milli: u16,
    pub minimum_reproducibility_milli: u16,
    pub minimum_replays: usize,
    pub minimum_qualified_windows: usize,
    pub maximum_failure_rate_milli: u16,
    pub maximum_contradiction_rate_milli: u16,
    pub maximum_regression_milli: u16,
    pub batch: FederatedBatchSchedule,
    pub observations: Vec<ContinualPromotionObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualPromotionWindow {
    pub window_id: String,
    pub window_index: usize,
    pub lower_epoch: u32,
    pub upper_epoch: u32,
    pub observation_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub observation_count: usize,
    pub qualified_count: usize,
    pub independent_group_count: usize,
    pub success_rate_milli: u16,
    pub failure_rate_milli: u16,
    pub contradiction_rate_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub disposition: PromotionWindowDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualPromotionDecision {
    pub capability_id: String,
    pub status: ContinualPromotionStatus,
    pub baseline_window_index: Option<usize>,
    pub recent_window_index: Option<usize>,
    pub baseline_success_rate_milli: u16,
    pub recent_success_rate_milli: u16,
    pub regression_milli: u16,
    pub recent_failure_rate_milli: u16,
    pub recent_contradiction_rate_milli: u16,
    pub qualified_window_count: usize,
    pub rollback_required: bool,
    pub reason: String,
    pub route: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualPromotionReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub current_epoch: u32,
    pub batch_digest: ContentHash,
    pub observation_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub omissions: BTreeMap<String, String>,
    pub windows: Vec<ContinualPromotionWindow>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub decision: ContinualPromotionDecision,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContinualPromotionError {
    #[error("continual promotion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("continual promotion input is invalid: {0}")]
    InvalidInput(String),
    #[error("continual promotion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("continual promotion digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn window_bounds(current_epoch: u32, window_size: u32, index: usize) -> (u32, u32) {
    let upper = current_epoch.saturating_sub(index as u32 * window_size);
    let lower = upper.saturating_sub(window_size.saturating_sub(1));
    (lower, upper)
}

fn window_id(index: usize, lower: u32, upper: u32) -> String {
    format!("window-{index:03}-{lower:08}-{upper:08}")
}

fn rate(numerator: usize, denominator: usize) -> u16 {
    if denominator == 0 {
        0
    } else {
        ((numerator as u64 * 1_000) / denominator as u64).min(1_000) as u16
    }
}

fn digest_input(output: &ContinualPromotionReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "capability_id": output.capability_id,
        "current_epoch": output.current_epoch,
        "batch_digest": output.batch_digest,
        "observation_order": output.observation_order,
        "selected_order": output.selected_order,
        "omitted_order": output.omitted_order,
        "omissions": output.omissions,
        "windows": output.windows,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "unknown_order": output.unknown_order,
        "failed_order": output.failed_order,
        "uncertainty": output.uncertainty,
        "decision": output.decision,
        "next_route": output.next_route,
    })
}

impl ContinualPromotionReport {
    pub fn validate(&self) -> Result<(), ContinualPromotionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.current_epoch == 0
            || self.batch_digest.as_str().len() != 64
            || !canonical(&self.observation_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.unknown_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.uncertainty)
            || self.windows.is_empty()
            || !canonical(
                &self
                    .windows
                    .iter()
                    .map(|window| window.window_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.next_route.trim().is_empty()
            || self.decision.capability_id != self.capability_id
            || self.decision.reason.trim().is_empty()
            || self.decision.route.trim().is_empty()
            || self.digest.as_str().len() != 64
        {
            return Err(ContinualPromotionError::InvalidOutput(
                "identity, ordering, window, decision, route, or digest fields are invalid".into(),
            ));
        }
        if self.observation_order.len() != self.selected_order.len() + self.omitted_order.len()
            || self
                .selected_order
                .iter()
                .any(|id| self.omitted_order.binary_search(id).is_ok())
            || self.windows.iter().any(|window| {
                window.observation_count != window.observation_order.len()
                    || window.qualified_count != window.qualified_order.len()
                    || !canonical(&window.observation_order)
                    || !canonical(&window.qualified_order)
                    || !canonical(&window.negative_order)
                    || !canonical(&window.contradicted_order)
                    || !canonical(&window.unknown_order)
                    || !canonical(&window.failed_order)
                    || window.qualified_count > window.observation_count
                    || window.success_rate_milli > 1_000
                    || window.failure_rate_milli > 1_000
                    || window.contradiction_rate_milli > 1_000
                    || window.quality_milli > 1_000
                    || window.reproducibility_milli > 1_000
            })
        {
            return Err(ContinualPromotionError::InvalidOutput(
                "observation partitions or window metrics are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ContinualPromotionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ContinualPromotionError::Digest(
                "continual promotion digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct WindowAccumulator {
    observation_order: Vec<String>,
    qualified_order: Vec<String>,
    negative_order: Vec<String>,
    contradicted_order: Vec<String>,
    unknown_order: Vec<String>,
    failed_order: Vec<String>,
    independent_groups: BTreeSet<String>,
    success_count: usize,
    failure_count: usize,
    contradiction_count: usize,
    quality_sum: u64,
    reproducibility_sum: u64,
}

fn route_for(status: ContinualPromotionStatus) -> &'static str {
    match status {
        ContinualPromotionStatus::Promote => "glioma_evidence_knowledge_bridge",
        ContinualPromotionStatus::Continue => "glioma_federated_batch_scheduler",
        ContinualPromotionStatus::Rollback => "glioma_federated_evidence_operating_cycle",
        ContinualPromotionStatus::Hold => "glioma_evidence_researcher_workbench",
    }
}

/// Evaluate scheduled cycle outcomes over bounded temporal windows. The decision is a promotion
/// recommendation for the declared capability only; it never certifies a biological mechanism or
/// authorizes a physical action.
pub fn evaluate_glioma_continual_promotion(
    request: &ContinualPromotionRequest,
) -> Result<ContinualPromotionReport, ContinualPromotionError> {
    if request.objective.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.current_epoch == 0
        || request.window_count < 2
        || request.window_count > MAX_WINDOWS
        || request.window_size_epochs == 0
        || request.minimum_observations_per_window == 0
        || request.minimum_independent_groups_per_window == 0
        || request.minimum_qualified_windows == 0
        || request.minimum_qualified_windows > request.window_count
        || request.minimum_quality_milli > 1_000
        || request.minimum_reproducibility_milli > 1_000
        || request.maximum_failure_rate_milli > 1_000
        || request.maximum_contradiction_rate_milli > 1_000
        || request.maximum_regression_milli > 1_000
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(ContinualPromotionError::InvalidRequest(
            "capability, temporal window, quality, replay, rate, or observation bounds are invalid"
                .into(),
        ));
    }
    if request.batch.objective != request.objective {
        return Err(ContinualPromotionError::InvalidRequest(
            "batch schedule objective does not match promotion objective".into(),
        ));
    }
    request
        .batch
        .validate()
        .map_err(|error| ContinualPromotionError::InvalidInput(error.to_string()))?;
    let selected_candidates = request
        .batch
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    if observations
        .windows(2)
        .any(|pair| pair[0].observation_id == pair[1].observation_id)
    {
        return Err(ContinualPromotionError::InvalidRequest(
            "observation identifiers must be unique".into(),
        ));
    }
    if observations.iter().any(|observation| {
        observation.observation_id.trim().is_empty()
            || observation.candidate_id.trim().is_empty()
            || observation.epoch == 0
            || observation.epoch > request.current_epoch
            || observation.quality_milli > 1_000
            || observation.reproducibility_milli > 1_000
            || observation.independent_groups == 0
            || observation.cost_units == 0
            || observation.result_digest.as_str().len() != 64
    }) {
        return Err(ContinualPromotionError::InvalidInput(
            "observation identity, epoch, quality, independence, cost, or result digest is invalid"
                .into(),
        ));
    }
    let mut omissions = BTreeMap::new();
    let horizon_lower = window_bounds(
        request.current_epoch,
        request.window_size_epochs,
        request.window_count - 1,
    )
    .0;
    let mut selected = Vec::new();
    for observation in &observations {
        if !selected_candidates.contains(&observation.candidate_id) {
            omissions.insert(
                observation.observation_id.clone(),
                "candidate_not_selected".into(),
            );
        } else if observation.epoch < horizon_lower {
            omissions.insert(
                observation.observation_id.clone(),
                "outside_window_horizon".into(),
            );
        } else {
            selected.push(observation.clone());
        }
    }
    let mut accumulators = (0..request.window_count)
        .map(|_| WindowAccumulator::default())
        .collect::<Vec<_>>();
    for observation in &selected {
        let index = (request.current_epoch - observation.epoch) / request.window_size_epochs;
        if index as usize >= request.window_count {
            omissions.insert(
                observation.observation_id.clone(),
                "outside_window_horizon".into(),
            );
            continue;
        }
        let accumulator = &mut accumulators[index as usize];
        accumulator
            .observation_order
            .push(observation.observation_id.clone());
        accumulator.independent_groups.extend(
            (0..observation.independent_groups)
                .map(|group| format!("{}::group-{group:04}", observation.candidate_id)),
        );
        if observation.quality_milli >= request.minimum_quality_milli
            && observation.reproducibility_milli >= request.minimum_reproducibility_milli
            && observation.replay_count >= request.minimum_replays
            && observation.independent_groups >= request.minimum_independent_groups_per_window
            && observation.outcome != ContinualOutcomeState::Unknown
        {
            accumulator
                .qualified_order
                .push(observation.observation_id.clone());
            accumulator.quality_sum += observation.quality_milli as u64;
            accumulator.reproducibility_sum += observation.reproducibility_milli as u64;
        }
        match observation.outcome {
            ContinualOutcomeState::Succeeded => accumulator.success_count += 1,
            ContinualOutcomeState::Failed => {
                accumulator.failure_count += 1;
                accumulator
                    .failed_order
                    .push(observation.observation_id.clone());
            }
            ContinualOutcomeState::Contradicted => {
                accumulator.contradiction_count += 1;
                accumulator
                    .contradicted_order
                    .push(observation.observation_id.clone());
            }
            ContinualOutcomeState::Negative => {
                accumulator
                    .negative_order
                    .push(observation.observation_id.clone());
            }
            ContinualOutcomeState::Unknown => {
                accumulator
                    .unknown_order
                    .push(observation.observation_id.clone());
            }
        }
    }
    let mut windows = Vec::with_capacity(request.window_count);
    for (index, mut accumulator) in accumulators.into_iter().enumerate() {
        accumulator.observation_order.sort();
        accumulator.qualified_order.sort();
        accumulator.negative_order.sort();
        accumulator.contradicted_order.sort();
        accumulator.unknown_order.sort();
        accumulator.failed_order.sort();
        let observation_count = accumulator.observation_order.len();
        let qualified_count = accumulator.qualified_order.len();
        let independent_group_count = accumulator.independent_groups.len();
        let (lower_epoch, upper_epoch) =
            window_bounds(request.current_epoch, request.window_size_epochs, index);
        let disposition = if observation_count >= request.minimum_observations_per_window
            && qualified_count >= request.minimum_observations_per_window
            && independent_group_count >= request.minimum_independent_groups_per_window
        {
            PromotionWindowDisposition::Qualified
        } else if observation_count == 0 {
            PromotionWindowDisposition::Unresolved
        } else {
            PromotionWindowDisposition::Partial
        };
        windows.push(ContinualPromotionWindow {
            window_id: window_id(index, lower_epoch, upper_epoch),
            window_index: index,
            lower_epoch,
            upper_epoch,
            observation_order: accumulator.observation_order,
            qualified_order: accumulator.qualified_order,
            negative_order: accumulator.negative_order,
            contradicted_order: accumulator.contradicted_order,
            unknown_order: accumulator.unknown_order,
            failed_order: accumulator.failed_order,
            observation_count,
            qualified_count,
            independent_group_count,
            success_rate_milli: rate(accumulator.success_count, observation_count),
            failure_rate_milli: rate(accumulator.failure_count, observation_count),
            contradiction_rate_milli: rate(accumulator.contradiction_count, observation_count),
            quality_milli: if qualified_count == 0 {
                0
            } else {
                (accumulator.quality_sum / qualified_count as u64) as u16
            },
            reproducibility_milli: if qualified_count == 0 {
                0
            } else {
                (accumulator.reproducibility_sum / qualified_count as u64) as u16
            },
            disposition,
        });
    }
    windows.sort_by(|left, right| left.window_id.cmp(&right.window_id));
    let qualified_window_indices = windows
        .iter()
        .filter(|window| window.disposition == PromotionWindowDisposition::Qualified)
        .map(|window| window.window_index)
        .collect::<Vec<_>>();
    let baseline = windows
        .iter()
        .rev()
        .find(|window| window.disposition == PromotionWindowDisposition::Qualified);
    let recent = windows
        .iter()
        .find(|window| window.disposition == PromotionWindowDisposition::Qualified);
    let baseline_success = baseline
        .map(|window| window.success_rate_milli)
        .unwrap_or(0);
    let recent_success = recent.map(|window| window.success_rate_milli).unwrap_or(0);
    let regression = baseline_success.saturating_sub(recent_success);
    let recent_failure = recent.map(|window| window.failure_rate_milli).unwrap_or(0);
    let recent_contradiction = recent
        .map(|window| window.contradiction_rate_milli)
        .unwrap_or(0);
    let has_unknown = windows
        .iter()
        .any(|window| !window.unknown_order.is_empty());
    let status = if recent_contradiction > request.maximum_contradiction_rate_milli
        || recent_failure > request.maximum_failure_rate_milli
        || (baseline.is_some() && regression > request.maximum_regression_milli)
    {
        if baseline.is_some() {
            ContinualPromotionStatus::Rollback
        } else {
            ContinualPromotionStatus::Hold
        }
    } else if qualified_window_indices.len() < request.minimum_qualified_windows {
        ContinualPromotionStatus::Hold
    } else if has_unknown
        || windows
            .iter()
            .any(|window| window.disposition != PromotionWindowDisposition::Qualified)
    {
        ContinualPromotionStatus::Continue
    } else if recent.is_some() {
        ContinualPromotionStatus::Promote
    } else {
        ContinualPromotionStatus::Hold
    };
    let reason = match status {
        ContinualPromotionStatus::Promote => "qualified temporal windows meet quality, replay, independence, failure, contradiction, and regression gates".into(),
        ContinualPromotionStatus::Continue => "the capability is actionable but unresolved or partially observed; gather the next declared window".into(),
        ContinualPromotionStatus::Rollback => "recent failure, contradiction, or regression exceeds the declared rollback threshold".into(),
        ContinualPromotionStatus::Hold => "promotion is underpowered or has unresolved/unknown outcomes; no pass is inferred".into(),
    };
    let next_route: String = route_for(status).into();
    let mut uncertainty = Vec::new();
    if !omissions.is_empty() {
        uncertainty.push("observations_were_omitted_from_the_declared_horizon_or_schedule".into());
    }
    if has_unknown {
        uncertainty.push("unknown_outcomes_remain_unresolved".into());
    }
    if qualified_window_indices.len() < request.minimum_qualified_windows {
        uncertainty.push("qualified_window_quorum_not_met".into());
    }
    uncertainty.sort();
    let observation_order = observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    let omitted_order = omissions.keys().cloned().collect::<Vec<_>>();
    let selected_order = selected
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let negative_order = windows
        .iter()
        .flat_map(|window| window.negative_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let contradicted_order = windows
        .iter()
        .flat_map(|window| window.contradicted_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let unknown_order = windows
        .iter()
        .flat_map(|window| window.unknown_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let failed_order = windows
        .iter()
        .flat_map(|window| window.failed_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let decision = ContinualPromotionDecision {
        capability_id: request.capability_id.clone(),
        status,
        baseline_window_index: baseline.map(|window| window.window_index),
        recent_window_index: recent.map(|window| window.window_index),
        baseline_success_rate_milli: baseline_success,
        recent_success_rate_milli: recent_success,
        regression_milli: regression,
        recent_failure_rate_milli: recent_failure,
        recent_contradiction_rate_milli: recent_contradiction,
        qualified_window_count: qualified_window_indices.len(),
        rollback_required: status == ContinualPromotionStatus::Rollback,
        reason,
        route: next_route.clone(),
    };
    let mut report = ContinualPromotionReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        current_epoch: request.current_epoch,
        batch_digest: request.batch.digest.clone(),
        observation_order,
        selected_order,
        omitted_order,
        omissions,
        windows,
        negative_order,
        contradicted_order,
        unknown_order,
        failed_order,
        uncertainty,
        decision,
        next_route,
        digest: ContentHash::of_bytes(b"unsealed-glioma-continual-promotion"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ContinualPromotionError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p01_evidence_surveillance::federated_batch_scheduler::{
        FederatedBatchActionDecision, FederatedBatchDecision, FederatedBatchDisposition,
    };

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn batch() -> FederatedBatchSchedule {
        let decision = FederatedBatchActionDecision {
            candidate_id: "candidate-1".into(),
            cycle_key: "cycle-1".into(),
            cycle_epoch: 1,
            action_id: "action-001".into(),
            rank: 1,
            route: "glioma_federated_evidence_operating_cycle".into(),
            priority_milli: 900,
            decision: FederatedBatchDecision::Selected,
            reason: "selected".into(),
        };
        let mut output = FederatedBatchSchedule {
            feature_id: super::super::federated_batch_scheduler::FEATURE_ID.into(),
            output_schema: super::super::federated_batch_scheduler::OUTPUT_SCHEMA.into(),
            objective: "continual promotion".into(),
            schedule_epoch: 1,
            input_cycle_digest_order: vec![hash("cycle")],
            cycle_order: vec!["cycle-1".into()],
            candidate_order: vec!["candidate-1".into()],
            selected_order: vec!["candidate-1".into()],
            deferred_order: Vec::new(),
            rejected_order: Vec::new(),
            decisions: vec![decision],
            selected_by_cycle: BTreeMap::from([(String::from("cycle-1"), 1)]),
            selected_by_route: BTreeMap::from([(
                String::from("glioma_federated_evidence_operating_cycle"),
                1,
            )]),
            selected_cycle_order: vec!["cycle-1".into()],
            selected_route_order: vec!["glioma_federated_evidence_operating_cycle".into()],
            eligible_cycle_count: 1,
            selected_action_count: 1,
            capacity_utilization_milli: 1_000,
            fairness_milli: 1_000,
            disposition: FederatedBatchDisposition::Ready,
            next_routes: vec!["glioma_federated_evidence_operating_cycle".into()],
            uncertainty: Vec::new(),
            digest: hash("pending"),
        };
        output.digest = ContentHash::of_value(
            &super::super::federated_batch_scheduler::digest_input(&output),
        )
        .unwrap();
        output
    }

    fn observation(
        id: &str,
        epoch: u32,
        outcome: ContinualOutcomeState,
    ) -> ContinualPromotionObservation {
        ContinualPromotionObservation {
            observation_id: id.into(),
            candidate_id: "candidate-1".into(),
            epoch,
            outcome,
            quality_milli: 900,
            reproducibility_milli: 900,
            independent_groups: 2,
            replay_count: 2,
            cost_units: 2,
            result_digest: hash(id),
        }
    }

    fn request(observations: Vec<ContinualPromotionObservation>) -> ContinualPromotionRequest {
        ContinualPromotionRequest {
            objective: "continual promotion".into(),
            capability_id: "glioma-cycle-capability".into(),
            current_epoch: 10,
            window_count: 2,
            window_size_epochs: 5,
            minimum_observations_per_window: 1,
            minimum_independent_groups_per_window: 2,
            minimum_quality_milli: 700,
            minimum_reproducibility_milli: 700,
            minimum_replays: 1,
            minimum_qualified_windows: 2,
            maximum_failure_rate_milli: 250,
            maximum_contradiction_rate_milli: 250,
            maximum_regression_milli: 250,
            batch: batch(),
            observations,
        }
    }

    #[test]
    fn qualified_windows_promote_capability() {
        let report = evaluate_glioma_continual_promotion(&request(vec![
            observation("old", 5, ContinualOutcomeState::Succeeded),
            observation("recent", 10, ContinualOutcomeState::Succeeded),
        ]))
        .unwrap();
        assert_eq!(report.decision.status, ContinualPromotionStatus::Promote);
        assert!(!report.decision.rollback_required);
        report.validate().unwrap();
    }

    #[test]
    fn contradiction_triggers_rollback_when_baseline_exists() {
        let report = evaluate_glioma_continual_promotion(&request(vec![
            observation("old", 5, ContinualOutcomeState::Succeeded),
            observation("recent", 10, ContinualOutcomeState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(report.decision.status, ContinualPromotionStatus::Rollback);
        assert!(report.decision.rollback_required);
        assert_eq!(report.contradicted_order, vec!["recent"]);
    }

    #[test]
    fn unknown_outcome_holds_without_inventing_a_pass() {
        let report = evaluate_glioma_continual_promotion(&request(vec![
            observation("old", 5, ContinualOutcomeState::Succeeded),
            observation("recent", 10, ContinualOutcomeState::Unknown),
        ]))
        .unwrap();
        assert_eq!(report.decision.status, ContinualPromotionStatus::Hold);
        assert_eq!(report.unknown_order, vec!["recent"]);
    }

    #[test]
    fn outside_horizon_is_omitted_deterministically() {
        let mut request = request(vec![
            observation("old", 5, ContinualOutcomeState::Succeeded),
            observation("recent", 10, ContinualOutcomeState::Succeeded),
            observation("outside", 1, ContinualOutcomeState::Succeeded),
        ]);
        request.window_size_epochs = 4;
        let first = evaluate_glioma_continual_promotion(&request).unwrap();
        let second = evaluate_glioma_continual_promotion(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.omitted_order, vec!["outside"]);
        assert_eq!(first.omissions["outside"], "outside_window_horizon");
    }
}
