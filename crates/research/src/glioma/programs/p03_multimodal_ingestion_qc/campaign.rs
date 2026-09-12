//! Bounded autonomous multimodal-ingestion and QC campaigns for preclinical glioma research.
//!
//! The campaign repeatedly evaluates metadata-only observations, turns missing coverage and
//! quality defects into typed local ingestion actions, and replans from returned observations.
//! It never imputes a missing modality, moves raw payloads, or turns a QC defect into a biological
//! conclusion.

use crate::glioma::multimodal::{
    harmonize_multimodal_inputs, MultimodalDisposition, MultimodalObservation, MultimodalQcReport,
    MultimodalRequest,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalIngestionCampaign1@1";
pub const MAX_ROUNDS: u16 = 24;
pub const MAX_ACTIONS_PER_ROUND: usize = 16;
pub const MAX_RETRIES: u8 = 6;
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionQcActionKind {
    AddMissingModality,
    AddMissingModelSystem,
    RepairExcludedObservation,
    ResolveQualityDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionQcAction {
    pub action_id: String,
    pub kind: IngestionQcActionKind,
    pub target: String,
    pub modality: Option<GliomaModality>,
    pub model_system: Option<GliomaModelSystem>,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionCampaignRequest {
    pub request: MultimodalRequest,
    pub observations: Vec<MultimodalObservation>,
    pub max_actions_per_round: usize,
    pub budget_units: u64,
    pub cost_per_action_units: u32,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultimodalIngestionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local parsers and catalog adapters implement this seam. The controller passes
/// metadata and receives new, de-identified observation descriptors only.
pub trait MultimodalIngestionCampaignExecutor {
    fn ingest_action(
        &mut self,
        request: &MultimodalRequest,
        action: &IngestionQcAction,
        report: &MultimodalQcReport,
        observations: &[MultimodalObservation],
        attempt: u8,
    ) -> Result<Vec<MultimodalObservation>, MultimodalIngestionExecutionFailure>;
}

/// Deterministic MCP sandbox adapter. It returns a single synthetic, compliant metadata row.
#[derive(Debug, Default)]
pub struct DryRunMultimodalIngestionCampaignExecutor;

impl MultimodalIngestionCampaignExecutor for DryRunMultimodalIngestionCampaignExecutor {
    fn ingest_action(
        &mut self,
        request: &MultimodalRequest,
        action: &IngestionQcAction,
        report: &MultimodalQcReport,
        observations: &[MultimodalObservation],
        attempt: u8,
    ) -> Result<Vec<MultimodalObservation>, MultimodalIngestionExecutionFailure> {
        let modality = action
            .modality
            .or_else(|| {
                request
                    .required_modalities
                    .iter()
                    .copied()
                    .find(|candidate| {
                        !report.comparable_order.iter().any(|id| {
                            observations.iter().any(|observation| {
                                observation.observation_id == *id
                                    && observation.modality == *candidate
                            })
                        })
                    })
            })
            .or_else(|| observations.first().map(|observation| observation.modality))
            .unwrap_or(GliomaModality::Genomics);
        let model_system = action
            .model_system
            .or_else(|| request.required_model_systems.iter().next().copied())
            .or_else(|| {
                observations
                    .first()
                    .map(|observation| observation.model_system)
            })
            .unwrap_or(GliomaModelSystem::Organoid);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| MultimodalIngestionExecutionFailure {
            reason: format!("dry-run ingestion digest failed: {error}"),
            retryable: false,
        })?;
        Ok(vec![MultimodalObservation {
            observation_id: format!("dry-run-ingestion:{}", action.action_id),
            study_id: request.study_id.clone(),
            sample_lineage: format!("dry-run-lineage:{}", action.action_id),
            modality,
            model_system,
            batch_id: "dry-run-batch".into(),
            coordinate_system: request.expected_coordinate_system.clone(),
            unit_system: request.expected_unit_system.clone(),
            missing_fraction_milli: 0,
            feature_count: 1,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-ingestion:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.multimodal-observation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_observation_order: Vec<String>,
    pub actions: Vec<IngestionQcAction>,
    pub report: MultimodalQcReport,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalIngestionCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalIngestionCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub rounds: Vec<MultimodalIngestionCampaignRound>,
    pub observations: Vec<MultimodalObservation>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub accepted_observation_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_report: MultimodalQcReport,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: MultimodalIngestionCampaignDisposition,
    pub stop_reason: MultimodalIngestionCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalIngestionCampaignError {
    #[error("multimodal ingestion campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal ingestion campaign planning failed: {0}")]
    Planning(String),
    #[error("multimodal ingestion campaign execution failed: {0}")]
    Execution(String),
    #[error("multimodal ingestion campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal ingestion campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &MultimodalIngestionCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "study_id": campaign.study_id,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "completed_order": campaign.completed_order,
        "failed_order": campaign.failed_order,
        "accepted_observation_order": campaign.accepted_observation_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_report": campaign.final_report,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_observations(
    observations: &[MultimodalObservation],
) -> Result<(), MultimodalIngestionCampaignError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(MultimodalIngestionCampaignError::InvalidRequest(
            "observation bound exceeded".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in observations {
        observation
            .artifact
            .validate()
            .map_err(|error| MultimodalIngestionCampaignError::InvalidRequest(error.to_string()))?;
        if observation.observation_id.trim().is_empty()
            || observation.study_id.trim().is_empty()
            || observation.sample_lineage.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.coordinate_system.trim().is_empty()
            || observation.unit_system.trim().is_empty()
            || observation.feature_count == 0
            || observation.missing_fraction_milli > 1_000
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(MultimodalIngestionCampaignError::InvalidRequest(
                "observation identity, metadata, feature count, missingness, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn plan_actions(
    request: &MultimodalIngestionCampaignRequest,
    report: &MultimodalQcReport,
) -> Result<Vec<IngestionQcAction>, MultimodalIngestionCampaignError> {
    let mut actions = Vec::new();
    for modality in &report.missing_modality_order {
        actions.push(IngestionQcAction {
            action_id: format!("qc:missing-modality:{modality:?}"),
            kind: IngestionQcActionKind::AddMissingModality,
            target: format!("{modality:?}"),
            modality: Some(*modality),
            model_system: request.request.required_model_systems.iter().next().copied(),
            priority_milli: 1_000,
            rationale: "required modality coverage is incomplete; acquire a local metadata-bound observation".into(),
        });
    }
    for model_system in &report.missing_model_order {
        actions.push(IngestionQcAction {
            action_id: format!("qc:missing-model:{model_system:?}"),
            kind: IngestionQcActionKind::AddMissingModelSystem,
            target: format!("{model_system:?}"),
            modality: request.request.required_modalities.iter().next().copied(),
            model_system: Some(*model_system),
            priority_milli: 950,
            rationale: "required preclinical model-system coverage is incomplete".into(),
        });
    }
    for observation_id in &report.excluded_order {
        actions.push(IngestionQcAction {
            action_id: format!("qc:excluded:{observation_id}"),
            kind: IngestionQcActionKind::RepairExcludedObservation,
            target: observation_id.clone(),
            modality: None,
            model_system: None,
            priority_milli: 900,
            rationale: "observation failed a declared coordinate, unit, missingness, or model gate"
                .into(),
        });
    }
    for defect in &report.defect_order {
        actions.push(IngestionQcAction {
            action_id: format!("qc:defect:{defect}"),
            kind: IngestionQcActionKind::ResolveQualityDefect,
            target: defect.clone(),
            modality: None,
            model_system: None,
            priority_milli: 850,
            rationale: "a metadata quality defect remains visible and requires local reconciliation".into(),
        });
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    actions.dedup_by(|left, right| left.action_id == right.action_id);
    actions.truncate(request.max_actions_per_round.min(MAX_ACTIONS_PER_ROUND));
    Ok(actions)
}

fn replace_observations(
    observations: &mut Vec<MultimodalObservation>,
    replacements: Vec<MultimodalObservation>,
) {
    for replacement in replacements {
        if let Some(existing) = observations
            .iter_mut()
            .find(|observation| observation.observation_id == replacement.observation_id)
        {
            *existing = replacement;
        } else {
            observations.push(replacement);
        }
    }
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
}

fn validate_returned_observations(
    request: &MultimodalRequest,
    observations: &[MultimodalObservation],
    action: &IngestionQcAction,
) -> Result<(), MultimodalIngestionCampaignError> {
    if observations.is_empty() {
        return Err(MultimodalIngestionCampaignError::Execution(format!(
            "executor returned no observations for {}",
            action.action_id
        )));
    }
    for observation in observations {
        if observation.study_id != request.study_id {
            return Err(MultimodalIngestionCampaignError::Execution(format!(
                "executor returned an observation for another study: {}",
                observation.observation_id
            )));
        }
    }
    validate_observations(observations).map_err(|error| {
        MultimodalIngestionCampaignError::Execution(format!(
            "executor returned invalid observations for {}: {error}",
            action.action_id
        ))
    })
}

fn validate_request(
    request: &MultimodalIngestionCampaignRequest,
) -> Result<MultimodalQcReport, MultimodalIngestionCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.cost_per_action_units == 0
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS_PER_ROUND
    {
        return Err(MultimodalIngestionCampaignError::InvalidRequest(
            "bounded rounds, retries, budget, action cost, and action count are required".into(),
        ));
    }
    validate_observations(&request.observations)?;
    harmonize_multimodal_inputs(&request.request, &request.observations)
        .map_err(|error| MultimodalIngestionCampaignError::InvalidRequest(error.to_string()))
}

impl MultimodalIngestionCampaign {
    pub fn validate(&self) -> Result<(), MultimodalIngestionCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.completed_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.accepted_observation_order)
            || self
                .completed_order
                .iter()
                .any(|id| self.failed_order.binary_search(id).is_ok())
        {
            return Err(MultimodalIngestionCampaignError::InvalidOutput(
                "identity, canonical partitions, or campaign fields are invalid".into(),
            ));
        }
        validate_observations(&self.observations)
            .map_err(|error| MultimodalIngestionCampaignError::InvalidOutput(error.to_string()))?;
        self.final_report
            .validate()
            .map_err(|error| MultimodalIngestionCampaignError::InvalidOutput(error.to_string()))?;
        let mut rounds = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !rounds.insert(round.round)
                || !canonical(&round.action_order)
                || !canonical(&round.completed_order)
                || !canonical(&round.failed_order)
                || !canonical(&round.accepted_observation_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(MultimodalIngestionCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            round.report.validate().map_err(|error| {
                MultimodalIngestionCampaignError::InvalidOutput(error.to_string())
            })?;
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(MultimodalIngestionCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalIngestionCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalIngestionCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute bounded QC/ingestion rounds, recomputing the report after every accepted local batch.
pub fn execute_glioma_multimodal_ingestion_campaign<E: MultimodalIngestionCampaignExecutor>(
    request: &MultimodalIngestionCampaignRequest,
    executor: &mut E,
) -> Result<MultimodalIngestionCampaign, MultimodalIngestionCampaignError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    let mut rounds = Vec::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = MultimodalIngestionCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let report = harmonize_multimodal_inputs(&request.request, &observations)
            .map_err(|error| MultimodalIngestionCampaignError::Planning(error.to_string()))?;
        negative.extend(report.negative_evidence.iter().cloned());
        uncertainty.extend(report.uncertainty.iter().cloned());
        let actions = plan_actions(request, &report)?;
        let eligible = actions
            .iter()
            .filter(|action| {
                !completed.contains(&action.action_id) && !failed.contains(&action.action_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if request.stop_on_qualified
            && report.disposition == MultimodalDisposition::Qualified
            && eligible.is_empty()
        {
            stop_reason = MultimodalIngestionCampaignStopReason::Qualified;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining < u64::from(request.cost_per_action_units) {
            stop_reason = MultimodalIngestionCampaignStopReason::BudgetExhausted;
            break;
        }
        if eligible.is_empty() {
            stop_reason = MultimodalIngestionCampaignStopReason::NoActions;
            break;
        }
        let max_batch = (remaining / u64::from(request.cost_per_action_units)) as usize;
        let selected = eligible
            .into_iter()
            .take(max_batch.clamp(1, MAX_ACTIONS_PER_ROUND))
            .collect::<Vec<_>>();
        let before_budget = remaining;
        let mut completed_round = Vec::new();
        let mut failed_round = Vec::new();
        let mut accepted_round = Vec::new();
        let mut retry_round = 0_u32;
        let mut progress = false;
        for action in &selected {
            let mut returned = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.ingest_action(
                    &request.request,
                    action,
                    &report,
                    &observations,
                    attempt,
                ) {
                    Ok(rows) => {
                        validate_returned_observations(&request.request, &rows, action)?;
                        returned = Some(rows);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(MultimodalIngestionCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            retry_round = retry_round.saturating_add(1);
                            continue;
                        }
                        failed_round.push(action.action_id.clone());
                        failed.insert(action.action_id.clone());
                        break;
                    }
                }
            }
            if let Some(rows) = returned {
                completed_round.push(action.action_id.clone());
                completed.insert(action.action_id.clone());
                for row in &rows {
                    accepted_round.push(row.observation_id.clone());
                    accepted.insert(row.observation_id.clone());
                }
                replace_observations(&mut observations, rows);
                progress = true;
            } else {
                break;
            }
        }
        completed_round.sort();
        failed_round.sort();
        accepted_round.sort();
        let cost = request
            .cost_per_action_units
            .saturating_mul(selected.len() as u32);
        budget_spent = budget_spent.saturating_add(u64::from(cost));
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated_report = harmonize_multimodal_inputs(&request.request, &observations)
            .map_err(|error| MultimodalIngestionCampaignError::Planning(error.to_string()))?;
        negative.extend(updated_report.negative_evidence.iter().cloned());
        uncertainty.extend(updated_report.uncertainty.iter().cloned());
        let mut action_order = selected
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>();
        action_order.sort();
        rounds.push(MultimodalIngestionCampaignRound {
            round: round_number,
            action_order,
            completed_order: completed_round,
            failed_order: failed_round.clone(),
            accepted_observation_order: accepted_round,
            actions: selected,
            report: updated_report.clone(),
            cost_units: cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: retry_round,
        });
        if !failed_round.is_empty() {
            stop_reason = MultimodalIngestionCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = MultimodalIngestionCampaignStopReason::NoProgress;
            break;
        }
        let next_actions = plan_actions(request, &updated_report)?;
        let next_eligible = next_actions.iter().any(|action| {
            !completed.contains(&action.action_id) && !failed.contains(&action.action_id)
        });
        if request.stop_on_qualified
            && updated_report.disposition == MultimodalDisposition::Qualified
            && !next_eligible
        {
            stop_reason = MultimodalIngestionCampaignStopReason::Qualified;
            break;
        }
        if after_budget == 0 {
            stop_reason = MultimodalIngestionCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_report = harmonize_multimodal_inputs(&request.request, &observations)
        .map_err(|error| MultimodalIngestionCampaignError::Planning(error.to_string()))?;
    negative.extend(final_report.negative_evidence.iter().cloned());
    uncertainty.extend(final_report.uncertainty.iter().cloned());
    let final_actions = plan_actions(request, &final_report)?;
    let final_eligible = final_actions.iter().any(|action| {
        !completed.contains(&action.action_id) && !failed.contains(&action.action_id)
    });
    if request.stop_on_qualified
        && final_report.disposition == MultimodalDisposition::Qualified
        && !final_eligible
    {
        stop_reason = MultimodalIngestionCampaignStopReason::Qualified;
    }
    let disposition = match stop_reason {
        MultimodalIngestionCampaignStopReason::Qualified => {
            MultimodalIngestionCampaignDisposition::Qualified
        }
        MultimodalIngestionCampaignStopReason::BudgetExhausted => {
            MultimodalIngestionCampaignDisposition::BudgetBlocked
        }
        MultimodalIngestionCampaignStopReason::ExecutorFailed => {
            MultimodalIngestionCampaignDisposition::Failed
        }
        MultimodalIngestionCampaignStopReason::NoActions => {
            MultimodalIngestionCampaignDisposition::NoActions
        }
        _ if final_report.disposition == MultimodalDisposition::Unresolved => {
            MultimodalIngestionCampaignDisposition::Unresolved
        }
        _ => MultimodalIngestionCampaignDisposition::Partial,
    };
    let mut output = MultimodalIngestionCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.request.study_id.clone(),
        rounds,
        observations,
        completed_order: completed.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        accepted_observation_order: accepted.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_report,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-ingestion-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalIngestionCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, modality: GliomaModality) -> MultimodalObservation {
        MultimodalObservation {
            observation_id: id.into(),
            study_id: "study-1".into(),
            sample_lineage: format!("sample-{id}"),
            modality,
            model_system: GliomaModelSystem::Organoid,
            batch_id: "batch-1".into(),
            coordinate_system: "pixel".into(),
            unit_system: "count".into(),
            missing_fraction_milli: 0,
            feature_count: 10,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> MultimodalIngestionCampaignRequest {
        MultimodalIngestionCampaignRequest {
            request: MultimodalRequest {
                study_id: "study-1".into(),
                required_modalities: BTreeSet::from([
                    GliomaModality::Genomics,
                    GliomaModality::Imaging,
                ]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                expected_coordinate_system: "pixel".into(),
                expected_unit_system: "count".into(),
                max_missing_fraction_milli: 100,
            },
            observations: vec![observation("seed", GliomaModality::Genomics)],
            max_actions_per_round: 4,
            budget_units: 2,
            cost_per_action_units: 1,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn campaign_adds_missing_modality_and_replans_to_qualified() {
        let request = request();
        let mut first_executor = DryRunMultimodalIngestionCampaignExecutor;
        let mut second_executor = DryRunMultimodalIngestionCampaignExecutor;
        let first =
            execute_glioma_multimodal_ingestion_campaign(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_multimodal_ingestion_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            MultimodalIngestionCampaignDisposition::Qualified
        );
        assert_eq!(
            first.final_report.disposition,
            MultimodalDisposition::Qualified
        );
        first.validate().unwrap();
    }

    #[test]
    fn quality_defect_remains_partial_instead_of_being_erased() {
        let mut request = request();
        request.request.required_modalities = BTreeSet::from([GliomaModality::Genomics]);
        request.observations[0].coordinate_system = "wrong".into();
        request.stop_on_qualified = false;
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let output = execute_glioma_multimodal_ingestion_campaign(&request, &mut executor).unwrap();
        assert_ne!(
            output.disposition,
            MultimodalIngestionCampaignDisposition::Qualified
        );
        assert!(!output.final_report.excluded_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit_for_multiple_missing_modalities() {
        let mut request = request();
        request.request.required_modalities = BTreeSet::from([
            GliomaModality::Genomics,
            GliomaModality::Imaging,
            GliomaModality::Proteomics,
        ]);
        request.budget_units = 1;
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let output = execute_glioma_multimodal_ingestion_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            MultimodalIngestionCampaignStopReason::BudgetExhausted
        );
        assert_eq!(
            output.disposition,
            MultimodalIngestionCampaignDisposition::BudgetBlocked
        );
        output.validate().unwrap();
    }
}
