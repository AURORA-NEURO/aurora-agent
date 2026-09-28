//! Beam-selected autonomous evidence-refresh campaigns for preclinical glioma research.
//!
//! Snapshot surveillance identifies changed, stale, contradictory, missing, or negative
//! evidence. This controller turns that queue into a bounded local workflow: it dispatches typed
//! refresh actions, replaces only the evidence record returned by the local provider, and reruns
//! surveillance after every round. Each round scores the complete refresh batch jointly for
//! priority and modality/model coverage. It does not fetch the internet or promote synthetic
//! dry-run records into biological conclusions.

use super::surveillance::{
    EvidenceSurveillance, EvidenceSurveillanceAction, EvidenceSurveillanceActionKind,
    EvidenceSurveillanceDisposition, EvidenceSurveillanceRequest, surveil_glioma_evidence,
};
use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceRefreshCampaign1@3";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_ACTIONS_PER_ROUND: usize = 16;
pub const MAX_RETRIES: u8 = 6;
const REFRESH_BATCH_BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRefreshCampaignRequest {
    pub surveillance: EvidenceSurveillanceRequest,
    pub previous_records: Vec<EvidenceRecord>,
    pub current_records: Vec<EvidenceRecord>,
    pub budget_units: u64,
    pub cost_per_action_units: u32,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRefreshExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local literature, dataset, assay, or replication adapters implement this seam.
/// The controller supplies only typed metadata and receives one validated local evidence record.
pub trait EvidenceRefreshCampaignExecutor {
    fn refresh_action(
        &mut self,
        action: &EvidenceSurveillanceAction,
        previous_records: &[EvidenceRecord],
        current_records: &[EvidenceRecord],
        attempt: u8,
    ) -> Result<EvidenceRecord, EvidenceRefreshExecutionFailure>;
}

/// Deterministic sandbox adapter. It produces local synthetic records for replay tests only.
#[derive(Debug, Default)]
pub struct DryRunEvidenceRefreshCampaignExecutor;

impl EvidenceRefreshCampaignExecutor for DryRunEvidenceRefreshCampaignExecutor {
    fn refresh_action(
        &mut self,
        action: &EvidenceSurveillanceAction,
        previous_records: &[EvidenceRecord],
        current_records: &[EvidenceRecord],
        attempt: u8,
    ) -> Result<EvidenceRecord, EvidenceRefreshExecutionFailure> {
        let source = current_records
            .iter()
            .chain(previous_records.iter())
            .find(|record| record.evidence_id == action.evidence_id)
            .ok_or_else(|| EvidenceRefreshExecutionFailure {
                reason: format!(
                    "no local source metadata for evidence {}",
                    action.evidence_id
                ),
                retryable: false,
            })?;
        let state = match action.kind {
            EvidenceSurveillanceActionKind::InvestigateContradiction => EvidenceState::Contradicted,
            EvidenceSurveillanceActionKind::RevalidateNegative => EvidenceState::Negative,
            EvidenceSurveillanceActionKind::ResolveUnknown => EvidenceState::Unknown,
            EvidenceSurveillanceActionKind::RefreshStale
            | EvidenceSurveillanceActionKind::ReviewNewEvidence
            | EvidenceSurveillanceActionKind::RestoreRemovedEvidence
            | EvidenceSurveillanceActionKind::ReassessScope => EvidenceState::Supported,
        };
        let artifact_hash = ContentHash::of_value(&serde_json::json!({
            "evidence_id": action.evidence_id,
            "action_id": action.action_id,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| EvidenceRefreshExecutionFailure {
            reason: format!("dry-run evidence digest failed: {error}"),
            retryable: false,
        })?;
        Ok(EvidenceRecord {
            evidence_id: source.evidence_id.clone(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-refresh:{}", action.evidence_id),
                content_hash: artifact_hash,
                content_type: "application/vnd.aurora.glioma.evidence-refresh+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: source.source_kind,
            claim: source.claim.clone(),
            scope: source.scope.clone(),
            modality: source.modality,
            model_system: source.model_system,
            state,
            relevance_milli: source.relevance_milli.max(700),
            quality_milli: source.quality_milli.max(700),
            reproducibility_milli: source.reproducibility_milli.max(700),
            release_epoch: source.release_epoch.saturating_add(1),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRefreshCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub refreshed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub surveillance: EvidenceSurveillance,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRefreshCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRefreshCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRefreshCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<EvidenceRefreshCampaignRound>,
    pub previous_records: Vec<EvidenceRecord>,
    pub current_records: Vec<EvidenceRecord>,
    pub refreshed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_surveillance: EvidenceSurveillance,
    pub deferred_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: EvidenceRefreshCampaignDisposition,
    pub stop_reason: EvidenceRefreshCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceRefreshCampaignError {
    #[error("evidence-refresh campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-refresh campaign planning failed: {0}")]
    Planning(String),
    #[error("evidence-refresh campaign execution failed: {0}")]
    Execution(String),
    #[error("evidence-refresh campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-refresh campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &EvidenceRefreshCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "previous_records": campaign.previous_records,
        "current_records": campaign.current_records,
        "refreshed_order": campaign.refreshed_order,
        "failed_order": campaign.failed_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_surveillance": campaign.final_surveillance,
        "deferred_action_order": campaign.deferred_action_order,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_records(records: &[EvidenceRecord]) -> Result<(), EvidenceRefreshCampaignError> {
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| EvidenceRefreshCampaignError::InvalidRequest(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceRefreshCampaignError::InvalidRequest(
                "evidence identity, claim, scope, scores, artifact, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn validate_request(
    request: &EvidenceRefreshCampaignRequest,
) -> Result<(), EvidenceRefreshCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.cost_per_action_units == 0
        || request.surveillance.max_actions == 0
        || request.surveillance.max_actions > MAX_ACTIONS_PER_ROUND
    {
        return Err(EvidenceRefreshCampaignError::InvalidRequest(
            "bounded rounds, retries, budget, action cost, and surveillance actions are required"
                .into(),
        ));
    }
    validate_records(&request.previous_records)?;
    validate_records(&request.current_records)?;
    surveil_glioma_evidence(
        &request.surveillance,
        &request.previous_records,
        &request.current_records,
    )
    .map_err(|error| EvidenceRefreshCampaignError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn replace_record(records: &mut Vec<EvidenceRecord>, replacement: EvidenceRecord) {
    if let Some(existing) = records
        .iter_mut()
        .find(|record| record.evidence_id == replacement.evidence_id)
    {
        *existing = replacement;
    } else {
        records.push(replacement);
    }
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
}

fn validate_returned_record(
    record: &EvidenceRecord,
    action: &EvidenceSurveillanceAction,
) -> Result<(), EvidenceRefreshCampaignError> {
    record
        .source_artifact
        .validate()
        .map_err(|error| EvidenceRefreshCampaignError::Execution(error.to_string()))?;
    if record.evidence_id != action.evidence_id
        || record.claim.trim().is_empty()
        || record.scope.trim().is_empty()
        || record.relevance_milli > 1_000
        || record.quality_milli > 1_000
        || record.reproducibility_milli > 1_000
    {
        return Err(EvidenceRefreshCampaignError::Execution(format!(
            "executor returned an invalid record for evidence {}",
            action.evidence_id
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefreshBatchState {
    selected: Vec<usize>,
}

fn refresh_batch_score(state: &RefreshBatchState, eligible: &[&EvidenceSurveillanceAction]) -> u64 {
    let mut score = 0_u64;
    let mut covered_modalities = BTreeSet::new();
    let mut covered_models = BTreeSet::new();
    for index in &state.selected {
        let action = eligible[*index];
        score = score.saturating_add(u64::from(action.priority_milli).saturating_mul(1_000_000));
        for modality in &action.required_modalities {
            if covered_modalities.insert(*modality) {
                score = score.saturating_add(100_000_000);
            }
        }
        for model in &action.required_model_systems {
            if covered_models.insert(*model) {
                score = score.saturating_add(100_000_000);
            }
        }
    }
    score
}

fn select_refresh_actions<'a>(
    eligible: &[&'a EvidenceSurveillanceAction],
    max_batch: usize,
) -> Vec<&'a EvidenceSurveillanceAction> {
    if max_batch == 0 || eligible.is_empty() {
        return Vec::new();
    }
    let mut states = vec![RefreshBatchState {
        selected: Vec::new(),
    }];
    for index in 0..eligible.len() {
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() < max_batch {
                let mut selected = state.selected.clone();
                selected.push(index);
                next.push(RefreshBatchState { selected });
            }
        }
        next.sort_by(|left, right| {
            refresh_batch_score(right, eligible)
                .cmp(&refresh_batch_score(left, eligible))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(REFRESH_BATCH_BEAM_WIDTH);
        states = next;
    }
    let chosen = states
        .into_iter()
        .max_by(|left, right| {
            refresh_batch_score(left, eligible)
                .cmp(&refresh_batch_score(right, eligible))
                .then_with(|| right.selected.cmp(&left.selected))
        })
        .expect("refresh beam always retains an empty state");
    chosen
        .selected
        .into_iter()
        .map(|index| eligible[index])
        .collect()
}

impl EvidenceRefreshCampaign {
    pub fn validate(&self) -> Result<(), EvidenceRefreshCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.refreshed_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .refreshed_order
                .iter()
                .any(|id| self.failed_order.binary_search(id).is_ok())
        {
            return Err(EvidenceRefreshCampaignError::InvalidOutput(
                "identity, canonical partitions, or evidence fields are invalid".into(),
            ));
        }
        validate_records(&self.previous_records)
            .map_err(|error| EvidenceRefreshCampaignError::InvalidOutput(error.to_string()))?;
        validate_records(&self.current_records)
            .map_err(|error| EvidenceRefreshCampaignError::InvalidOutput(error.to_string()))?;
        self.final_surveillance
            .validate()
            .map_err(|error| EvidenceRefreshCampaignError::InvalidOutput(error.to_string()))?;
        if self.deferred_action_order != self.final_surveillance.deferred_action_order
            || self.deferred_action_order.iter().any(|id| {
                self.final_surveillance
                    .action_order
                    .iter()
                    .any(|selected| selected == id)
            })
        {
            return Err(EvidenceRefreshCampaignError::InvalidOutput(
                "campaign deferred frontier does not reconcile with final surveillance".into(),
            ));
        }
        let mut rounds = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !rounds.insert(round.round)
                || !canonical(&round.action_order)
                || !canonical(&round.refreshed_order)
                || !canonical(&round.failed_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(EvidenceRefreshCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(EvidenceRefreshCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceRefreshCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceRefreshCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute bounded evidence-refresh rounds. Each accepted local record replaces exactly one
/// evidence ID; every subsequent round recomputes the surveillance delta from the new snapshot.
pub fn execute_glioma_evidence_refresh_campaign<E: EvidenceRefreshCampaignExecutor>(
    request: &EvidenceRefreshCampaignRequest,
    executor: &mut E,
) -> Result<EvidenceRefreshCampaign, EvidenceRefreshCampaignError> {
    validate_request(request)?;
    let previous_records = request.previous_records.clone();
    let mut current_records = request.current_records.clone();
    let mut rounds = Vec::new();
    let mut refreshed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = EvidenceRefreshCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let surveillance =
            surveil_glioma_evidence(&request.surveillance, &previous_records, &current_records)
                .map_err(|error| EvidenceRefreshCampaignError::Planning(error.to_string()))?;
        // The caller's surveillance cap controls the public snapshot, not the planner's
        // visibility.  Recompile a complete local frontier so a refreshed/failed top action
        // cannot strand lower-priority evidence work behind the cap.  The execution cap below
        // still bounds every round.
        let mut frontier_request = request.surveillance.clone();
        frontier_request.max_actions = MAX_RECORDS;
        let planning_surveillance =
            surveil_glioma_evidence(&frontier_request, &previous_records, &current_records)
                .map_err(|error| EvidenceRefreshCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(surveillance.negative_evidence.iter().cloned());
        uncertainty.extend(surveillance.uncertainty.iter().cloned());
        if request.stop_on_qualified
            && surveillance.disposition == EvidenceSurveillanceDisposition::Qualified
            && planning_surveillance.actions.is_empty()
        {
            stop_reason = EvidenceRefreshCampaignStopReason::Qualified;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining < u64::from(request.cost_per_action_units) {
            stop_reason = EvidenceRefreshCampaignStopReason::BudgetExhausted;
            break;
        }
        let eligible = planning_surveillance
            .actions
            .iter()
            .filter(|action| {
                !refreshed.contains(&action.action_id) && !failed.contains(&action.action_id)
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            stop_reason = EvidenceRefreshCampaignStopReason::NoActions;
            break;
        }
        let max_batch = (remaining / u64::from(request.cost_per_action_units)) as usize;
        let round_cap = request.surveillance.max_actions.min(MAX_ACTIONS_PER_ROUND);
        let selected = select_refresh_actions(&eligible, max_batch.clamp(1, round_cap));
        let before_budget = remaining;
        let mut refreshed_round = Vec::new();
        let mut failed_round = Vec::new();
        let mut retry_round = 0_u32;
        let mut progress = false;
        for action in selected.iter().copied() {
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.refresh_action(action, &previous_records, &current_records, attempt)
                {
                    Ok(record) => {
                        validate_returned_record(&record, action)?;
                        accepted = Some(record);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(EvidenceRefreshCampaignError::Execution(
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
            if let Some(record) = accepted {
                refreshed_round.push(record.evidence_id.clone());
                refreshed.insert(action.action_id.clone());
                replace_record(&mut current_records, record);
                progress = true;
            } else {
                break;
            }
        }
        refreshed_round.sort();
        failed_round.sort();
        let cost = request
            .cost_per_action_units
            .saturating_mul(selected.len() as u32);
        budget_spent = budget_spent.saturating_add(u64::from(cost));
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated_surveillance =
            surveil_glioma_evidence(&request.surveillance, &previous_records, &current_records)
                .map_err(|error| EvidenceRefreshCampaignError::Planning(error.to_string()))?;
        let mut action_order = selected
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>();
        action_order.sort();
        rounds.push(EvidenceRefreshCampaignRound {
            round: round_number,
            action_order,
            refreshed_order: refreshed_round,
            failed_order: failed_round.clone(),
            surveillance: updated_surveillance.clone(),
            cost_units: cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: retry_round,
        });
        // Keep the last acknowledged baseline stable while a bounded campaign drains its
        // frontier.  Advancing it to the partially refreshed snapshot would erase still-
        // unexecuted stale/contradictory transitions before their deferred actions are promoted.
        if !failed_round.is_empty() {
            stop_reason = EvidenceRefreshCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = EvidenceRefreshCampaignStopReason::NoProgress;
            break;
        }
        if request.stop_on_qualified
            && updated_surveillance.disposition == EvidenceSurveillanceDisposition::Qualified
            && updated_surveillance.actions.is_empty()
            && updated_surveillance.deferred_action_order.is_empty()
        {
            stop_reason = EvidenceRefreshCampaignStopReason::Qualified;
            break;
        }
        if after_budget == 0 {
            stop_reason = EvidenceRefreshCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_surveillance =
        surveil_glioma_evidence(&request.surveillance, &previous_records, &current_records)
            .map_err(|error| EvidenceRefreshCampaignError::Planning(error.to_string()))?;
    negative_evidence.extend(final_surveillance.negative_evidence.iter().cloned());
    uncertainty.extend(final_surveillance.uncertainty.iter().cloned());
    if request.stop_on_qualified
        && final_surveillance.disposition == EvidenceSurveillanceDisposition::Qualified
        && final_surveillance.actions.is_empty()
        && final_surveillance.deferred_action_order.is_empty()
    {
        stop_reason = EvidenceRefreshCampaignStopReason::Qualified;
    }
    let disposition = match stop_reason {
        EvidenceRefreshCampaignStopReason::Qualified => {
            EvidenceRefreshCampaignDisposition::Qualified
        }
        EvidenceRefreshCampaignStopReason::BudgetExhausted => {
            EvidenceRefreshCampaignDisposition::BudgetBlocked
        }
        EvidenceRefreshCampaignStopReason::ExecutorFailed => {
            EvidenceRefreshCampaignDisposition::Failed
        }
        EvidenceRefreshCampaignStopReason::NoActions => {
            EvidenceRefreshCampaignDisposition::NoActions
        }
        _ if current_records.is_empty() => EvidenceRefreshCampaignDisposition::Unresolved,
        _ => EvidenceRefreshCampaignDisposition::Partial,
    };
    let mut output = EvidenceRefreshCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.surveillance.objective.clone(),
        rounds,
        previous_records,
        current_records,
        refreshed_order: refreshed.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        deferred_action_order: final_surveillance.deferred_action_order.clone(),
        final_surveillance,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-refresh-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceRefreshCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn record(id: &str, state: EvidenceState, epoch: u32) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma.evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Literature,
            claim: format!("claim-{id}"),
            scope: "preclinical glioma invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 800,
            quality_milli: 800,
            reproducibility_milli: 800,
            release_epoch: epoch,
        }
    }

    fn request() -> EvidenceRefreshCampaignRequest {
        EvidenceRefreshCampaignRequest {
            surveillance: EvidenceSurveillanceRequest {
                objective: "refresh glioma invasion evidence".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_priority_milli: 500,
                max_actions: 4,
                score_shift_threshold_milli: 20,
            },
            previous_records: vec![record("e1", EvidenceState::Supported, 1)],
            current_records: vec![record("e1", EvidenceState::Stale, 1)],
            budget_units: 2,
            cost_per_action_units: 1,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    fn action(
        id: &str,
        priority: u16,
        modality: GliomaModality,
        model_system: GliomaModelSystem,
    ) -> EvidenceSurveillanceAction {
        EvidenceSurveillanceAction {
            action_id: id.into(),
            evidence_id: format!("evidence-{id}"),
            kind: EvidenceSurveillanceActionKind::RefreshStale,
            priority_milli: priority,
            required_modalities: vec![modality],
            required_model_systems: vec![model_system],
            rationale: "test action".into(),
        }
    }

    #[test]
    fn refresh_batch_prefers_new_modality_and_model_over_redundant_priority() {
        let first = action(
            "high",
            1_000,
            GliomaModality::Genomics,
            GliomaModelSystem::Organoid,
        );
        let redundant = action(
            "redundant",
            999,
            GliomaModality::Genomics,
            GliomaModelSystem::Organoid,
        );
        let complementary = action(
            "complementary",
            950,
            GliomaModality::Imaging,
            GliomaModelSystem::MouseModel,
        );
        let actions = vec![&first, &redundant, &complementary];
        let selected = select_refresh_actions(&actions, 2);
        assert_eq!(
            selected
                .iter()
                .map(|action| action.action_id.as_str())
                .collect::<Vec<_>>(),
            vec!["high", "complementary"]
        );
    }

    #[test]
    fn campaign_refreshes_stale_evidence_and_replans_deterministically() {
        let request = request();
        let mut first_executor = DryRunEvidenceRefreshCampaignExecutor;
        let mut second_executor = DryRunEvidenceRefreshCampaignExecutor;
        let first =
            execute_glioma_evidence_refresh_campaign(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_evidence_refresh_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            EvidenceRefreshCampaignDisposition::Qualified
        );
        assert_eq!(first.current_records[0].state, EvidenceState::Supported);
        first.validate().unwrap();
    }

    #[test]
    fn campaign_preserves_contradiction_and_does_not_claim_qualification() {
        let mut request = request();
        request.previous_records[0].state = EvidenceState::Supported;
        request.current_records[0].state = EvidenceState::Contradicted;
        request.stop_on_qualified = false;
        let mut executor = DryRunEvidenceRefreshCampaignExecutor;
        let output = execute_glioma_evidence_refresh_campaign(&request, &mut executor).unwrap();
        assert!(
            output
                .negative_evidence
                .iter()
                .any(|value| value.contains("e1"))
        );
        assert_ne!(
            output.disposition,
            EvidenceRefreshCampaignDisposition::Qualified
        );
    }

    #[test]
    fn campaign_promotes_deferred_frontier_after_the_top_action_is_refreshed() {
        let mut request = request();
        request.surveillance.max_actions = 1;
        request.previous_records = vec![
            record("e1", EvidenceState::Supported, 1),
            record("e2", EvidenceState::Supported, 1),
        ];
        request.current_records = vec![
            record("e1", EvidenceState::Stale, 1),
            record("e2", EvidenceState::Stale, 1),
        ];
        request.budget_units = 2;
        let mut executor = DryRunEvidenceRefreshCampaignExecutor;
        let output = execute_glioma_evidence_refresh_campaign(&request, &mut executor).unwrap();

        assert_eq!(
            output.refreshed_order,
            vec![
                "surveillance:e1:RefreshStale",
                "surveillance:e2:RefreshStale"
            ]
        );
        assert_eq!(output.rounds.len(), 2);
        assert!(output.deferred_action_order.is_empty());
        assert_eq!(
            output.final_surveillance.deferred_action_order,
            output.deferred_action_order
        );
        output.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit() {
        let mut request = request();
        request.budget_units = 0;
        let mut executor = DryRunEvidenceRefreshCampaignExecutor;
        let error = execute_glioma_evidence_refresh_campaign(&request, &mut executor).unwrap_err();
        assert!(error.to_string().contains("budget"));
    }
}
