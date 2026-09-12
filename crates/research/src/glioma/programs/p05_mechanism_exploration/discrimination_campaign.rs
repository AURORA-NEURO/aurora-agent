//! Autonomous mechanism-discrimination campaigns for preclinical glioma research.
//!
//! This controller closes the loop around P05's mechanism discriminator: it selects the next
//! information-bearing feature, dispatches it through an institution-local adapter, replaces or
//! appends the returned typed observation, and recomputes the competing-mechanism ranking. A
//! planned assay is never treated as a result; diffuse posteriors, missing features, negative
//! evidence, retries, budget exhaustion, and no-progress states remain first-class output.

use super::discrimination::{
    discriminate_mechanisms, MechanismDiscrimination, MechanismDiscriminationDisposition,
    MechanismDiscriminationRequest, MechanismDiscriminatorAction, MechanismFeatureObservation,
    MechanismHypothesis,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismDiscriminationCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminationCampaignRequest {
    pub discrimination: MechanismDiscriminationRequest,
    pub hypotheses: Vec<MechanismHypothesis>,
    pub actions: Vec<MechanismDiscriminatorAction>,
    pub observations: Vec<MechanismFeatureObservation>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanismDiscriminationCampaignExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, imaging, omics, or computational adapters implement this seam.
/// Only a validated, de-identified feature observation may return to the controller.
pub trait MechanismDiscriminationCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &MechanismDiscriminatorAction,
        discrimination: &MechanismDiscrimination,
        attempt: u8,
    ) -> Result<MechanismFeatureObservation, MechanismDiscriminationCampaignExecutionFailure>;
}

/// Deterministic MCP sandbox adapter. It emits a local metadata-only observation at the mean of
/// the declared mechanism predictions; this is a workflow rehearsal, not biological evidence.
#[derive(Debug, Default)]
pub struct DryRunMechanismDiscriminationCampaignExecutor;

impl MechanismDiscriminationCampaignExecutor for DryRunMechanismDiscriminationCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &MechanismDiscriminatorAction,
        _discrimination: &MechanismDiscrimination,
        attempt: u8,
    ) -> Result<MechanismFeatureObservation, MechanismDiscriminationCampaignExecutionFailure> {
        let count = action.predicted_milli_by_mechanism.len().max(1) as i64;
        let observed_milli = action
            .predicted_milli_by_mechanism
            .values()
            .copied()
            .sum::<i64>()
            .saturating_div(count);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "feature_id": action.feature_id,
            "observed_milli": observed_milli,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| MechanismDiscriminationCampaignExecutionFailure {
            reason: format!("dry-run mechanism observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(MechanismFeatureObservation {
            feature_id: action.feature_id.clone(),
            observed_milli,
            uncertainty_milli: action.measurement_uncertainty_milli,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-mechanism-discrimination:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.mechanism-observation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminationCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub action_id: String,
    pub accepted_feature_order: Vec<String>,
    pub discrimination: MechanismDiscrimination,
    pub retry_count: u32,
    pub cost_units: u64,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDiscriminationCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDiscriminationCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminationCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<MechanismDiscriminationCampaignRound>,
    pub observations: Vec<MechanismFeatureObservation>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub accepted_feature_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_discrimination: MechanismDiscrimination,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: MechanismDiscriminationCampaignDisposition,
    pub stop_reason: MechanismDiscriminationCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismDiscriminationCampaignError {
    #[error("mechanism-discrimination campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism-discrimination campaign planning failed: {0}")]
    Planning(String),
    #[error("mechanism-discrimination campaign execution failed: {0}")]
    Execution(String),
    #[error("mechanism-discrimination campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism-discrimination campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &MechanismDiscriminationCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "accepted_feature_order": campaign.accepted_feature_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_discrimination": campaign.final_discrimination,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_observations(
    observations: &[MechanismFeatureObservation],
) -> Result<(), MechanismDiscriminationCampaignError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(MechanismDiscriminationCampaignError::InvalidRequest(
            "mechanism observation bound exceeded".into(),
        ));
    }
    let mut features = BTreeSet::new();
    for observation in observations {
        observation.artifact.validate().map_err(|error| {
            MechanismDiscriminationCampaignError::InvalidRequest(error.to_string())
        })?;
        if observation.feature_id.trim().is_empty()
            || observation.uncertainty_milli == 0
            || observation.uncertainty_milli > 1_000_000
            || observation.observed_milli.unsigned_abs() > 1_000_000
            || !features.insert(observation.feature_id.clone())
        {
            return Err(MechanismDiscriminationCampaignError::InvalidRequest(
                "mechanism observation identity, bounds, artifact, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_request(
    request: &MechanismDiscriminationCampaignRequest,
) -> Result<MechanismDiscrimination, MechanismDiscriminationCampaignError> {
    if request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
    {
        return Err(MechanismDiscriminationCampaignError::InvalidRequest(
            "positive budget and bounded rounds/retries are required".into(),
        ));
    }
    validate_observations(&request.observations)?;
    discriminate_mechanisms(
        &request.discrimination,
        &request.hypotheses,
        &request.observations,
        &request.actions,
    )
    .map_err(|error| MechanismDiscriminationCampaignError::InvalidRequest(error.to_string()))
}

fn replace_observation(
    observations: &mut Vec<MechanismFeatureObservation>,
    replacement: MechanismFeatureObservation,
) {
    if let Some(existing) = observations
        .iter_mut()
        .find(|observation| observation.feature_id == replacement.feature_id)
    {
        *existing = replacement;
    } else {
        observations.push(replacement);
    }
    observations.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
}

fn validate_returned_observation(
    observation: &MechanismFeatureObservation,
    action: &MechanismDiscriminatorAction,
) -> Result<(), MechanismDiscriminationCampaignError> {
    observation
        .artifact
        .validate()
        .map_err(|error| MechanismDiscriminationCampaignError::Execution(error.to_string()))?;
    if observation.feature_id != action.feature_id
        || observation.uncertainty_milli == 0
        || observation.uncertainty_milli > 1_000_000
        || observation.observed_milli.unsigned_abs() > 1_000_000
    {
        return Err(MechanismDiscriminationCampaignError::Execution(format!(
            "executor returned an observation outside action {} target contract",
            action.action_id
        )));
    }
    Ok(())
}

impl MechanismDiscriminationCampaign {
    pub fn validate(&self) -> Result<(), MechanismDiscriminationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || !canonical(&self.accepted_feature_order)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
        {
            return Err(MechanismDiscriminationCampaignError::InvalidOutput(
                "identity, canonical partitions, or campaign fields are invalid".into(),
            ));
        }
        validate_observations(&self.observations).map_err(|error| {
            MechanismDiscriminationCampaignError::InvalidOutput(error.to_string())
        })?;
        self.final_discrimination.validate().map_err(|error| {
            MechanismDiscriminationCampaignError::InvalidOutput(error.to_string())
        })?;
        if self.final_discrimination.objective != self.objective {
            return Err(MechanismDiscriminationCampaignError::InvalidOutput(
                "final discrimination objective does not reconcile".into(),
            ));
        }
        let mut rounds = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !rounds.insert(round.round)
                || !canonical(&round.action_order)
                || !round
                    .selected_action_order
                    .iter()
                    .all(|id| round.action_order.binary_search(id).is_ok())
                || round.action_order.binary_search(&round.action_id).is_err()
                || round.budget_after_units > round.budget_before_units
                || round.cost_units != round.budget_before_units - round.budget_after_units
                || round.action_order != round.discrimination.action_order
                || round.selected_action_order != round.discrimination.selected_action_order
                || round
                    .accepted_feature_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(MechanismDiscriminationCampaignError::InvalidOutput(
                    "round ordering, target, budget, or discrimination invariants are invalid"
                        .into(),
                ));
            }
            round.discrimination.validate().map_err(|error| {
                MechanismDiscriminationCampaignError::InvalidOutput(error.to_string())
            })?;
            spent = spent.saturating_add(round.cost_units);
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(MechanismDiscriminationCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismDiscriminationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismDiscriminationCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute bounded mechanism-discrimination rounds, recomputing competing-mechanism fit after
/// each returned local feature observation.
pub fn execute_glioma_mechanism_discrimination_campaign<
    E: MechanismDiscriminationCampaignExecutor,
>(
    request: &MechanismDiscriminationCampaignRequest,
    executor: &mut E,
) -> Result<MechanismDiscriminationCampaign, MechanismDiscriminationCampaignError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    let action_map = request
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut accepted_features = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u64;
    let mut stop_reason = MechanismDiscriminationCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let discrimination = discriminate_mechanisms(
            &request.discrimination,
            &request.hypotheses,
            &observations,
            &request.actions,
        )
        .map_err(|error| MechanismDiscriminationCampaignError::Planning(error.to_string()))?;
        negative.extend(discrimination.negative_evidence.iter().cloned());
        uncertainty.extend(discrimination.uncertainty.iter().cloned());
        let eligible = discrimination
            .selected_action_order
            .iter()
            .filter(|id| !completed.contains(*id) && !failed.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        if request.stop_on_qualified
            && discrimination.disposition == MechanismDiscriminationDisposition::Qualified
            && eligible.is_empty()
        {
            stop_reason = MechanismDiscriminationCampaignStopReason::Qualified;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining == 0 {
            stop_reason = MechanismDiscriminationCampaignStopReason::BudgetExhausted;
            break;
        }
        let Some(action_id) = eligible.first() else {
            stop_reason = MechanismDiscriminationCampaignStopReason::NoActions;
            break;
        };
        let action = action_map.get(action_id).ok_or_else(|| {
            MechanismDiscriminationCampaignError::Planning(format!(
                "selected action {action_id} is absent from action registry"
            ))
        })?;
        let cost = u64::from(action.cost_units);
        if cost > remaining {
            stop_reason = MechanismDiscriminationCampaignStopReason::BudgetExhausted;
            break;
        }
        let before_budget = remaining;
        let mut accepted = None;
        let mut round_retries = 0_u32;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_action(action, &discrimination, attempt) {
                Ok(observation) => {
                    validate_returned_observation(&observation, action)?;
                    accepted = Some(observation);
                    break;
                }
                Err(error) => {
                    if error.reason.trim().is_empty() {
                        return Err(MechanismDiscriminationCampaignError::Execution(
                            "executor returned an empty failure reason".into(),
                        ));
                    }
                    if error.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        round_retries = round_retries.saturating_add(1);
                        continue;
                    }
                    failed.insert(action.action_id.clone());
                    break;
                }
            }
        }
        budget_spent = budget_spent.saturating_add(cost);
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let mut accepted_feature_order = Vec::new();
        if let Some(observation) = accepted {
            accepted_feature_order.push(observation.feature_id.clone());
            accepted_features.insert(observation.feature_id.clone());
            replace_observation(&mut observations, observation);
            completed.insert(action.action_id.clone());
        }
        accepted_feature_order.sort();
        rounds.push(MechanismDiscriminationCampaignRound {
            round: round_number,
            action_order: discrimination.action_order.clone(),
            selected_action_order: discrimination.selected_action_order.clone(),
            action_id: action.action_id.clone(),
            accepted_feature_order,
            discrimination,
            retry_count: round_retries,
            cost_units: cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
        });
        if failed.contains(&action.action_id) {
            stop_reason = MechanismDiscriminationCampaignStopReason::ExecutorFailed;
            break;
        }
        if !completed.contains(&action.action_id) {
            stop_reason = MechanismDiscriminationCampaignStopReason::NoProgress;
            break;
        }
        if after_budget == 0 {
            stop_reason = MechanismDiscriminationCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_discrimination = discriminate_mechanisms(
        &request.discrimination,
        &request.hypotheses,
        &observations,
        &request.actions,
    )
    .map_err(|error| MechanismDiscriminationCampaignError::Planning(error.to_string()))?;
    negative.extend(final_discrimination.negative_evidence.iter().cloned());
    uncertainty.extend(final_discrimination.uncertainty.iter().cloned());
    let final_eligible = final_discrimination
        .selected_action_order
        .iter()
        .any(|id| !completed.contains(id) && !failed.contains(id));
    if request.stop_on_qualified
        && final_discrimination.disposition == MechanismDiscriminationDisposition::Qualified
        && !final_eligible
    {
        stop_reason = MechanismDiscriminationCampaignStopReason::Qualified;
    }
    let disposition = match stop_reason {
        MechanismDiscriminationCampaignStopReason::Qualified => {
            MechanismDiscriminationCampaignDisposition::Qualified
        }
        MechanismDiscriminationCampaignStopReason::BudgetExhausted => {
            MechanismDiscriminationCampaignDisposition::BudgetBlocked
        }
        MechanismDiscriminationCampaignStopReason::ExecutorFailed => {
            MechanismDiscriminationCampaignDisposition::Failed
        }
        MechanismDiscriminationCampaignStopReason::NoActions
            if final_discrimination.disposition
                == MechanismDiscriminationDisposition::Unresolved =>
        {
            MechanismDiscriminationCampaignDisposition::Unresolved
        }
        MechanismDiscriminationCampaignStopReason::NoActions => {
            MechanismDiscriminationCampaignDisposition::NoActions
        }
        _ if final_discrimination.disposition == MechanismDiscriminationDisposition::Unresolved => {
            MechanismDiscriminationCampaignDisposition::Unresolved
        }
        _ => MechanismDiscriminationCampaignDisposition::Partial,
    };
    let mut output = MechanismDiscriminationCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.discrimination.objective.clone(),
        rounds,
        observations,
        completed_action_order: completed.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        accepted_feature_order: accepted_features.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_discrimination,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-discrimination-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismDiscriminationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::discrimination::MechanismDiscriminatorAction;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn request() -> MechanismDiscriminationCampaignRequest {
        MechanismDiscriminationCampaignRequest {
            discrimination: MechanismDiscriminationRequest {
                objective: "resolve glioma invasion mechanisms".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_features: 2,
                max_mechanisms: 4,
                max_actions: 2,
                min_information_gain_milli: 10,
            },
            hypotheses: vec![
                MechanismHypothesis {
                    mechanism_id: "motility".into(),
                    statement: "motility drives invasion".into(),
                    predictions: vec![
                        super::super::discrimination::MechanismPrediction {
                            feature_id: "f1".into(),
                            predicted_milli: 100,
                            uncertainty_milli: 10,
                        },
                        super::super::discrimination::MechanismPrediction {
                            feature_id: "f2".into(),
                            predicted_milli: 200,
                            uncertainty_milli: 10,
                        },
                    ],
                },
                MechanismHypothesis {
                    mechanism_id: "matrix".into(),
                    statement: "matrix remodeling drives invasion".into(),
                    predictions: vec![
                        super::super::discrimination::MechanismPrediction {
                            feature_id: "f1".into(),
                            predicted_milli: 400,
                            uncertainty_milli: 10,
                        },
                        super::super::discrimination::MechanismPrediction {
                            feature_id: "f2".into(),
                            predicted_milli: 500,
                            uncertainty_milli: 10,
                        },
                    ],
                },
            ],
            actions: vec![MechanismDiscriminatorAction {
                action_id: "measure-f1".into(),
                feature_id: "f1".into(),
                predicted_milli_by_mechanism: std::collections::BTreeMap::from([
                    ("matrix".into(), 500),
                    ("motility".into(), 100),
                ]),
                measurement_uncertainty_milli: 20,
                feasibility_milli: 1_000,
                cost_units: 1,
            }],
            observations: vec![
                MechanismFeatureObservation {
                    feature_id: "f1".into(),
                    observed_milli: 100,
                    uncertainty_milli: 10,
                    artifact: LocalArtifactRef {
                        artifact_id: "seed".into(),
                        content_hash: hash("seed"),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                },
                MechanismFeatureObservation {
                    feature_id: "f2".into(),
                    observed_milli: 200,
                    uncertainty_milli: 10,
                    artifact: LocalArtifactRef {
                        artifact_id: "seed-f2".into(),
                        content_hash: hash("seed-f2"),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                },
            ],
            budget_units: 1,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn campaign_replans_discrimination_after_a_local_measurement() {
        let request = request();
        let mut first_executor = DryRunMechanismDiscriminationCampaignExecutor;
        let mut second_executor = DryRunMechanismDiscriminationCampaignExecutor;
        let first = execute_glioma_mechanism_discrimination_campaign(&request, &mut first_executor)
            .unwrap();
        let second =
            execute_glioma_mechanism_discrimination_campaign(&request, &mut second_executor)
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.completed_action_order, vec!["measure-f1"]);
        assert_eq!(first.rounds.len(), 1);
        assert_eq!(
            first.stop_reason,
            MechanismDiscriminationCampaignStopReason::Qualified
        );
        first.validate().unwrap();
    }

    #[test]
    fn missing_observations_remain_unresolved_not_confident() {
        let mut request = request();
        request.observations.clear();
        request.budget_units = 1;
        let mut executor = DryRunMechanismDiscriminationCampaignExecutor;
        let output =
            execute_glioma_mechanism_discrimination_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            MechanismDiscriminationCampaignDisposition::Unresolved
        );
        assert!(output
            .final_discrimination
            .negative_evidence
            .iter()
            .any(|item| item.contains("no-local")));
        output.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit_for_costly_discriminator() {
        let mut request = request();
        request.actions[0].cost_units = 2;
        request.budget_units = 1;
        let mut executor = DryRunMechanismDiscriminationCampaignExecutor;
        let output =
            execute_glioma_mechanism_discrimination_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            MechanismDiscriminationCampaignStopReason::BudgetExhausted
        );
        assert_eq!(
            output.disposition,
            MechanismDiscriminationCampaignDisposition::BudgetBlocked
        );
        output.validate().unwrap();
    }
}
