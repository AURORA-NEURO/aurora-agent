//! Bounded, multi-round execution of a glioma replication-closure frontier sequence.
//!
//! A frontier is a ranked decision for one evidence state.  Real research needs a campaign
//! controller that can consume a declared sequence of frontiers, reserve a global budget, stop
//! when a result is qualified/negative/unsafe, and preserve every partial or held round.  This
//! feature provides that controller without inventing a new observation or granting authority to
//! an agent: each round still passes through the guarded P10 closure executor.

use super::replication_closure_execution::{
    execute_glioma_replication_closure, ReplicationClosureExecutionDisposition,
    ReplicationClosureExecutionError, ReplicationClosureExecutionRequest,
    ReplicationClosureExecutionRun,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationClosureCampaign1@1";
pub const MAX_ROUNDS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureCampaignRequest {
    pub campaign_id: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub frontiers: Vec<ReplicationClosureExecutionRequest>,
    pub budget_units: u32,
    pub max_rounds: u16,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
    pub stop_on_unresolved: bool,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationClosureCampaignDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
    Held,
    Blocked,
    BudgetExhausted,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationClosureCampaignStopReason {
    Qualified,
    Negative,
    Unresolved,
    HeldByFrontier,
    Blocked,
    BudgetExhausted,
    MaxRounds,
    NoFrontiers,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureCampaignRound {
    pub round: u16,
    pub frontier_digest: ContentHash,
    pub selected_action_order: Vec<String>,
    pub reserved_budget_units: u32,
    pub execution: ReplicationClosureExecutionRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub campaign_id: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub rounds: Vec<ReplicationClosureCampaignRound>,
    pub completed_action_order: Vec<String>,
    pub held_round_order: Vec<u16>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub budget_reserved_units: u32,
    pub budget_used_units: u32,
    pub remaining_budget_units: u32,
    pub disposition: ReplicationClosureCampaignDisposition,
    pub stop_reason: ReplicationClosureCampaignStopReason,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationClosureCampaignError {
    #[error("replication closure campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication closure round failed: {0}")]
    Execution(#[from] ReplicationClosureExecutionError),
    #[error("replication closure campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication closure campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(run: &ReplicationClosureCampaignRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "campaign_id": run.campaign_id,
        "objective": run.objective,
        "model_system": run.model_system,
        "replay_identity": run.replay_identity,
        "rounds": run.rounds,
        "completed_action_order": run.completed_action_order,
        "held_round_order": run.held_round_order,
        "negative_evidence": run.negative_evidence,
        "uncertainty": run.uncertainty,
        "budget_reserved_units": run.budget_reserved_units,
        "budget_used_units": run.budget_used_units,
        "remaining_budget_units": run.remaining_budget_units,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "next_action": run.next_action,
        "boundary": run.boundary,
    })
}

fn validate_request(
    request: &ReplicationClosureCampaignRequest,
) -> Result<(), ReplicationClosureCampaignError> {
    if request.campaign_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.frontiers.is_empty()
        || request.frontiers.len() > MAX_ROUNDS
        || request.budget_units == 0
        || request.max_rounds == 0
        || usize::from(request.max_rounds) > MAX_ROUNDS
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ReplicationClosureCampaignError::InvalidRequest(
            "campaign identity, objective, at least one bounded frontier, budget, rounds, and replay identity are required".into(),
        ));
    }
    for frontier in &request.frontiers {
        if frontier.frontier.objective != request.objective
            || frontier.frontier.model_system != request.model_system
            || frontier.campaign.objective != request.objective
            || frontier.campaign.model_system != request.model_system
            || frontier.campaign.target_model_system != request.model_system
        {
            return Err(ReplicationClosureCampaignError::InvalidRequest(
                "every frontier and campaign must match the campaign objective and model system"
                    .into(),
            ));
        }
        frontier
            .frontier
            .validate()
            .map_err(|error| ReplicationClosureCampaignError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

impl ReplicationClosureCampaignRun {
    pub fn validate(&self) -> Result<(), ReplicationClosureCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.campaign_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS
            || !canonical(&self.completed_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.next_action.trim().is_empty()
            || self.budget_used_units > self.budget_reserved_units
            || self.remaining_budget_units
                != self
                    .budget_reserved_units
                    .saturating_sub(self.budget_used_units)
        {
            return Err(ReplicationClosureCampaignError::InvalidOutput(
                "identity, ordering, budget, boundary, or next-action invariant failed".into(),
            ));
        }
        let mut actions = BTreeSet::new();
        let mut reserved = 0u32;
        let mut used = 0u32;
        for round in &self.rounds {
            if round.execution.frontier_digest != round.frontier_digest
                || round.selected_action_order != round.execution.selected_action_order
                || round.execution.validate().is_err()
            {
                return Err(ReplicationClosureCampaignError::InvalidOutput(
                    "round digest, action binding, execution, or duplicate-action invariant failed"
                        .into(),
                ));
            }
            if round
                .execution
                .executable_action_order
                .iter()
                .any(|action| !actions.insert(action.clone()))
            {
                return Err(ReplicationClosureCampaignError::InvalidOutput(
                    "round digest, action binding, execution, or duplicate-action invariant failed"
                        .into(),
                ));
            }
            reserved = reserved.saturating_add(round.reserved_budget_units);
            used = used.saturating_add(
                round
                    .execution
                    .campaign
                    .as_ref()
                    .map(|campaign| campaign.budget_used_units)
                    .unwrap_or(0),
            );
        }
        if reserved != self.budget_reserved_units || used != self.budget_used_units {
            return Err(ReplicationClosureCampaignError::InvalidOutput(
                "campaign budget totals do not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReplicationClosureCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReplicationClosureCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a bounded sequence of validated closure frontiers.  The sequence is caller-declared,
/// but admission, execution, stop logic, and budget reconciliation remain deterministic.
pub fn execute_glioma_replication_closure_campaign<
    E: super::campaign::GliomaReplicationCampaignExecutor,
>(
    request: &ReplicationClosureCampaignRequest,
    executor: &mut E,
) -> Result<ReplicationClosureCampaignRun, ReplicationClosureCampaignError> {
    validate_request(request)?;
    let mut rounds = Vec::new();
    let mut completed_action_order = BTreeSet::new();
    let mut held_round_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut budget_reserved_units = 0u32;
    let mut budget_used_units = 0u32;
    let mut disposition = ReplicationClosureCampaignDisposition::Completed;
    let mut stop_reason = ReplicationClosureCampaignStopReason::Completed;
    let mut next_action = "publish the completed closure campaign and its replay evidence";

    for (index, frontier_request) in request.frontiers.iter().enumerate() {
        if index >= usize::from(request.max_rounds) {
            disposition = ReplicationClosureCampaignDisposition::Partial;
            stop_reason = ReplicationClosureCampaignStopReason::MaxRounds;
            next_action = "supply the next bounded frontier in a new campaign edition";
            break;
        }
        let reserved = frontier_request.campaign.budget_units;
        if budget_reserved_units.saturating_add(reserved) > request.budget_units {
            disposition = ReplicationClosureCampaignDisposition::BudgetExhausted;
            stop_reason = ReplicationClosureCampaignStopReason::BudgetExhausted;
            uncertainty.push(format!("frontier round {} withheld because its budget reservation exceeds the campaign cap", index + 1));
            next_action = "increase the campaign budget or split the remaining frontier into a new bounded campaign";
            break;
        }
        budget_reserved_units = budget_reserved_units.saturating_add(reserved);
        let execution = execute_glioma_replication_closure(frontier_request, executor)?;
        budget_used_units = budget_used_units.saturating_add(
            execution
                .campaign
                .as_ref()
                .map(|campaign| campaign.budget_used_units)
                .unwrap_or(0),
        );
        completed_action_order.extend(execution.executable_action_order.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.clone());
        uncertainty.extend(execution.uncertainty.clone());
        let round = ReplicationClosureCampaignRound {
            round: u16::try_from(index + 1).unwrap_or(u16::MAX),
            frontier_digest: execution.frontier_digest.clone(),
            selected_action_order: execution.selected_action_order.clone(),
            reserved_budget_units: reserved,
            execution: execution.clone(),
        };
        rounds.push(round);
        match execution.disposition {
            ReplicationClosureExecutionDisposition::Qualified if request.stop_on_qualified => {
                disposition = ReplicationClosureCampaignDisposition::Qualified;
                stop_reason = ReplicationClosureCampaignStopReason::Qualified;
                next_action = "hold the qualified closure campaign for independent methods review";
                break;
            }
            ReplicationClosureExecutionDisposition::Negative if request.stop_on_negative => {
                disposition = ReplicationClosureCampaignDisposition::Negative;
                stop_reason = ReplicationClosureCampaignStopReason::Negative;
                next_action =
                    "publish the negative closure result and preserve its boundary conditions";
                break;
            }
            ReplicationClosureExecutionDisposition::Unresolved if request.stop_on_unresolved => {
                disposition = ReplicationClosureCampaignDisposition::Unresolved;
                stop_reason = ReplicationClosureCampaignStopReason::Unresolved;
                next_action =
                    "resolve missing or contradictory evidence before another closure wave";
                break;
            }
            ReplicationClosureExecutionDisposition::HeldByFrontier => {
                disposition = ReplicationClosureCampaignDisposition::Held;
                stop_reason = ReplicationClosureCampaignStopReason::HeldByFrontier;
                held_round_order.push(u16::try_from(index + 1).unwrap_or(u16::MAX));
                next_action =
                    "repair or replace the held frontier before dispatching another round";
                break;
            }
            ReplicationClosureExecutionDisposition::Blocked => {
                disposition = ReplicationClosureCampaignDisposition::Blocked;
                stop_reason = ReplicationClosureCampaignStopReason::Blocked;
                next_action =
                    "repair the institution-local execution or policy boundary before retrying";
                break;
            }
            ReplicationClosureExecutionDisposition::Executed
            | ReplicationClosureExecutionDisposition::Partial
            | ReplicationClosureExecutionDisposition::Qualified
            | ReplicationClosureExecutionDisposition::Negative
            | ReplicationClosureExecutionDisposition::Unresolved => {
                disposition = ReplicationClosureCampaignDisposition::Partial;
                stop_reason = ReplicationClosureCampaignStopReason::MaxRounds;
                next_action = "continue with the next declared closure frontier or open a new campaign edition";
            }
        }
    }
    if rounds.is_empty() {
        disposition = ReplicationClosureCampaignDisposition::Held;
        stop_reason = ReplicationClosureCampaignStopReason::NoFrontiers;
        next_action = "provide at least one validated closure frontier";
    } else if rounds.len() == request.frontiers.len()
        && matches!(
            stop_reason,
            ReplicationClosureCampaignStopReason::Completed
                | ReplicationClosureCampaignStopReason::MaxRounds
        )
    {
        stop_reason = ReplicationClosureCampaignStopReason::Completed;
        if matches!(
            disposition,
            ReplicationClosureCampaignDisposition::Completed
        ) {
            next_action = "publish the completed closure campaign and its replay evidence";
        }
    }
    let mut run = ReplicationClosureCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        campaign_id: request.campaign_id.clone(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        rounds,
        completed_action_order: completed_action_order.into_iter().collect(),
        held_round_order,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        budget_reserved_units,
        budget_used_units,
        remaining_budget_units: request.budget_units.saturating_sub(budget_used_units),
        disposition,
        stop_reason,
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-closure-campaign"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| ReplicationClosureCampaignError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::super::replication_closure_frontier::{
        ReplicationClosureDisposition, ReplicationClosureScore, ReplicationClosureTarget,
    };
    use super::super::validation_replication_campaign::ValidationReplicationCampaignDisposition;
    use super::*;

    fn held_request() -> ReplicationClosureCampaignRequest {
        let mut frontier = super::super::replication_closure_frontier::ReplicationClosureFrontier {
            feature_id: "GAF-GLIOMA-P10-F27".into(),
            output_schema: "GliomaReplicationClosureFrontier1@1".into(),
            objective: "replicate organoid invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            source_replication_digest: ContentHash::of_bytes(b"replication"),
            source_disposition: ValidationReplicationCampaignDisposition::BlockedByValidation,
            source_stop_reason: None,
            candidate_order: vec!["hold".into()],
            scores: vec![ReplicationClosureScore {
                action_id: "hold".into(),
                route: "glioma_validation_replication_campaign_execute".into(),
                model_system: GliomaModelSystem::Organoid,
                target: ReplicationClosureTarget::ConfirmNegativeResult,
                utility_milli: 700,
                evidence_gap_milli: 900,
                risk_adjusted_information_milli: 700,
                cost_units: 1,
                decision: "blocked_by_replication_state".into(),
            }],
            selected_order: Vec::new(),
            deferred_order: Vec::new(),
            blocked_order: vec!["hold".into()],
            negative_evidence: vec!["upstream-blocked".into()],
            uncertainty: vec!["awaiting validation".into()],
            disposition: ReplicationClosureDisposition::Blocked,
            next_operator_action: "complete validation".into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let frontier_input = serde_json::json!({
            "feature_id": frontier.feature_id,
            "output_schema": frontier.output_schema,
            "objective": frontier.objective,
            "model_system": frontier.model_system,
            "source_replication_digest": frontier.source_replication_digest,
            "source_disposition": frontier.source_disposition,
            "source_stop_reason": frontier.source_stop_reason,
            "candidate_order": frontier.candidate_order,
            "scores": frontier.scores,
            "selected_order": frontier.selected_order,
            "deferred_order": frontier.deferred_order,
            "blocked_order": frontier.blocked_order,
            "negative_evidence": frontier.negative_evidence,
            "uncertainty": frontier.uncertainty,
            "disposition": frontier.disposition,
            "next_operator_action": frontier.next_operator_action,
        });
        frontier.digest = ContentHash::of_value(&frontier_input).unwrap();
        let campaign = super::super::campaign::GliomaReplicationCampaignRequest {
            objective: "replicate organoid invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![0, 0],
            min_sites: 1,
            min_replicates_per_site: 1,
            min_studies: 1,
            min_replicates_per_study: 1,
            effect_threshold_milli: 10,
            max_heterogeneity_milli: 500,
            max_i2_milli: 900,
            min_signal_to_noise_milli: 1,
            max_leave_one_out_shift_milli: 500,
            min_quality_milli: 500,
            distance_scale_milli: 1_000,
            max_transport_gap_milli: 500,
            max_transport_heterogeneity_milli: 500,
            budget_units: 1,
            max_rounds: 1,
            max_actions_per_round: 1,
            max_retries: 0,
            initial_studies: Vec::new(),
            initial_transport_studies: Vec::new(),
            replay_identity: ContentHash::of_bytes(b"replay"),
        };
        ReplicationClosureCampaignRequest {
            campaign_id: "closure-campaign".into(),
            objective: "replicate organoid invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            frontiers: vec![ReplicationClosureExecutionRequest { frontier, campaign }],
            budget_units: 1,
            max_rounds: 1,
            stop_on_qualified: true,
            stop_on_negative: true,
            stop_on_unresolved: true,
            replay_identity: ContentHash::of_bytes(b"campaign-replay"),
        }
    }

    #[test]
    fn held_frontier_stops_campaign_without_dispatch() {
        let mut executor =
            super::super::campaign::DryRunGliomaReplicationCampaignExecutor::default();
        let run =
            execute_glioma_replication_closure_campaign(&held_request(), &mut executor).unwrap();
        assert_eq!(run.disposition, ReplicationClosureCampaignDisposition::Held);
        assert_eq!(
            run.stop_reason,
            ReplicationClosureCampaignStopReason::HeldByFrontier
        );
        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].execution.campaign.is_none());
        run.validate().unwrap();
    }
}
