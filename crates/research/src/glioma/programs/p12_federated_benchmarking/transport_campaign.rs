//! Adaptive federated mechanism-transport campaign for preclinical glioma research.
//!
//! The transport estimator is useful only as a snapshot.  This feature makes it operational:
//! it ranks aggregate-only follow-up site actions from the current heterogeneity, asks an
//! institution-local worker for one typed site summary at a time, adds the summary, and reruns
//! transport analysis.  Raw traces, identities, instruments, and clinical data remain outside
//! the federation boundary.

use super::mechanism_transport::{
    analyze_federated_mechanism_transport, FederatedMechanismSite,
    FederatedMechanismTransportAnalysis, FederatedMechanismTransportDisposition,
    FederatedMechanismTransportError, FederatedMechanismTransportRequest,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedMechanismTransportCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_ACTIONS: usize = 256;
pub const MAX_RETRIES: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportAction {
    pub action_id: String,
    pub target_site_id: Option<String>,
    pub model_system: GliomaModelSystem,
    pub population_signature: Vec<i64>,
    pub cost_units: u32,
    pub expected_information_milli: u64,
    pub expected_effect_milli: i64,
    pub expected_heterogeneity_reduction_milli: u64,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub requested_replicates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportCampaignRequest {
    pub transport: FederatedMechanismTransportRequest,
    pub initial_sites: Vec<FederatedMechanismSite>,
    pub actions: Vec<FederatedMechanismTransportAction>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedMechanismTransportExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

pub trait FederatedMechanismTransportExecutor {
    fn execute_action(
        &mut self,
        action: &FederatedMechanismTransportAction,
        request: &FederatedMechanismTransportRequest,
        attempt: u8,
    ) -> Result<FederatedMechanismSite, FederatedMechanismTransportExecutionFailure>;
}

#[derive(Debug, Default)]
pub struct DryRunFederatedMechanismTransportExecutor;

impl FederatedMechanismTransportExecutor for DryRunFederatedMechanismTransportExecutor {
    fn execute_action(
        &mut self,
        action: &FederatedMechanismTransportAction,
        request: &FederatedMechanismTransportRequest,
        attempt: u8,
    ) -> Result<FederatedMechanismSite, FederatedMechanismTransportExecutionFailure> {
        let target = action.target_site_id.as_deref().unwrap_or("aggregate");
        let site_id = format!("dry-run:{}:{}", action.action_id, target);
        let study_id = format!("dry-run-study:{}", action.action_id);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "site_id": site_id,
            "study_id": study_id,
            "action_id": action.action_id,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| FederatedMechanismTransportExecutionFailure {
            reason: format!("dry-run aggregate digest failed: {error}"),
            retryable: false,
        })?;
        let population_signature =
            if action.population_signature.len() == request.target_signature.len() {
                action.population_signature.clone()
            } else {
                request.target_signature.clone()
            };
        Ok(FederatedMechanismSite {
            site_id,
            study_id,
            mechanism_id: request.mechanism_id.clone(),
            model_system: action.model_system,
            effect_milli: action.expected_effect_milli,
            uncertainty_milli: 100,
            quality_milli: 900,
            replicate_count: action.requested_replicates.max(1),
            population_signature,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-transport:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.transport+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub returned_site_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub transport_disposition: FederatedMechanismTransportDisposition,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedMechanismTransportCampaignDisposition {
    Qualified,
    Heterogeneous,
    Negative,
    Partial,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedMechanismTransportCampaignStopReason {
    Qualified,
    Negative,
    Heterogeneous,
    BudgetExhausted,
    NoEligibleActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismTransportCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<FederatedMechanismTransportCampaignRound>,
    pub sites: Vec<FederatedMechanismSite>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_transport: FederatedMechanismTransportAnalysis,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedMechanismTransportCampaignDisposition,
    pub stop_reason: FederatedMechanismTransportCampaignStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedMechanismTransportCampaignError {
    #[error("federated mechanism transport campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated mechanism transport analysis failed: {0}")]
    Transport(#[from] FederatedMechanismTransportError),
    #[error("federated mechanism transport execution failed: {0}")]
    Execution(String),
    #[error("federated mechanism transport campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated mechanism transport campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.iter().any(|value| value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(campaign: &FederatedMechanismTransportCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "sites": campaign.sites,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_transport": campaign.final_transport,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
        "next_step": campaign.next_step,
    })
}

fn validate_request(
    request: &FederatedMechanismTransportCampaignRequest,
) -> Result<(), FederatedMechanismTransportCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.initial_sites.is_empty()
    {
        return Err(FederatedMechanismTransportCampaignError::InvalidRequest(
            "bounded rounds/retries/budget, initial aggregate sites, and actions are required"
                .into(),
        ));
    }
    let mut action_ids = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || !action_ids.insert(action.action_id.clone())
            || action.cost_units == 0
            || action.feasibility_milli > 1_000
            || action.risk_milli > 1_000
            || action.requested_replicates == 0
            || action.population_signature.len() > 256
        {
            return Err(FederatedMechanismTransportCampaignError::InvalidRequest(
                "action identity, uniqueness, cost, feasibility, risk, signature, and replicate bounds are invalid".into(),
            ));
        }
    }
    analyze_federated_mechanism_transport(&request.transport, &request.initial_sites)?;
    Ok(())
}

fn action_score(
    action: &FederatedMechanismTransportAction,
    transport: &FederatedMechanismTransportAnalysis,
) -> u128 {
    let pressure = match transport.disposition {
        FederatedMechanismTransportDisposition::Heterogeneous => 1_800_u128,
        FederatedMechanismTransportDisposition::Unresolved => 1_500,
        FederatedMechanismTransportDisposition::Negative => 1_250,
        FederatedMechanismTransportDisposition::Partial => 1_100,
        FederatedMechanismTransportDisposition::Qualified => 300,
    };
    let value = u128::from(action.expected_information_milli)
        .saturating_add(u128::from(action.expected_heterogeneity_reduction_milli))
        .saturating_add(action.expected_effect_milli.unsigned_abs() as u128)
        .saturating_mul(pressure)
        .saturating_mul(u128::from(action.feasibility_milli.max(1)));
    let penalty = u128::from(action.cost_units)
        .saturating_mul(u128::from(1_000_u16.saturating_add(action.risk_milli)));
    value.saturating_sub(penalty)
}

fn validate_returned_site(
    site: &FederatedMechanismSite,
    action: &FederatedMechanismTransportAction,
    request: &FederatedMechanismTransportRequest,
    existing: &[FederatedMechanismSite],
) -> Result<(), FederatedMechanismTransportCampaignError> {
    if existing
        .iter()
        .any(|old| old.site_id == site.site_id || old.study_id == site.study_id)
    {
        return Err(FederatedMechanismTransportCampaignError::Execution(
            "executor returned a duplicate site or study identity".into(),
        ));
    }
    if !site.artifact.local_only
        || site.artifact.contains_human_data
        || site.artifact.contains_direct_identifiers
    {
        return Err(FederatedMechanismTransportCampaignError::Execution(
            "federated transport accepts only local, non-human aggregate artifacts".into(),
        ));
    }
    if site.model_system != action.model_system
        || (!action.population_signature.is_empty()
            && site.population_signature != action.population_signature)
    {
        return Err(FederatedMechanismTransportCampaignError::Execution(
            "executor returned a site outside the requested model/signature binding".into(),
        ));
    }
    let mut candidate = existing.to_vec();
    candidate.push(site.clone());
    analyze_federated_mechanism_transport(request, &candidate)
        .map_err(|error| FederatedMechanismTransportCampaignError::Execution(error.to_string()))?;
    Ok(())
}

impl FederatedMechanismTransportCampaign {
    pub fn validate(&self) -> Result<(), FederatedMechanismTransportCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.final_transport.objective != self.objective
            || self.rounds.len() > usize::from(MAX_ROUNDS)
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
        {
            return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                "identity, transport binding, bounds, or action partitions are invalid".into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut seen_sites = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || !unique_nonempty(&round.action_order)
                || !canonical(&round.returned_site_order)
                || !canonical(&round.failed_action_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            for site_id in &round.returned_site_order {
                if !seen_sites.insert(site_id.clone()) {
                    return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                        "a returned site was emitted in multiple rounds".into(),
                    ));
                }
            }
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                "budget or retry totals do not reconcile with rounds".into(),
            ));
        }
        let mut site_ids = BTreeSet::new();
        let mut study_ids = BTreeSet::new();
        for site in &self.sites {
            if !site_ids.insert(site.site_id.clone()) || !study_ids.insert(site.study_id.clone()) {
                return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                    "campaign sites must have unique site and study identities".into(),
                ));
            }
        }
        self.final_transport.validate().map_err(|error| {
            FederatedMechanismTransportCampaignError::InvalidOutput(error.to_string())
        })?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedMechanismTransportCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedMechanismTransportCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn disposition(
    transport: FederatedMechanismTransportDisposition,
) -> FederatedMechanismTransportCampaignDisposition {
    match transport {
        FederatedMechanismTransportDisposition::Qualified => {
            FederatedMechanismTransportCampaignDisposition::Qualified
        }
        FederatedMechanismTransportDisposition::Heterogeneous => {
            FederatedMechanismTransportCampaignDisposition::Heterogeneous
        }
        FederatedMechanismTransportDisposition::Negative => {
            FederatedMechanismTransportCampaignDisposition::Negative
        }
        FederatedMechanismTransportDisposition::Partial => {
            FederatedMechanismTransportCampaignDisposition::Partial
        }
        FederatedMechanismTransportDisposition::Unresolved => {
            FederatedMechanismTransportCampaignDisposition::Unresolved
        }
    }
}

pub fn execute_federated_mechanism_transport_campaign<E: FederatedMechanismTransportExecutor>(
    request: &FederatedMechanismTransportCampaignRequest,
    executor: &mut E,
) -> Result<FederatedMechanismTransportCampaign, FederatedMechanismTransportCampaignError> {
    validate_request(request)?;
    let mut sites = request.initial_sites.clone();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = FederatedMechanismTransportCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let transport = analyze_federated_mechanism_transport(&request.transport, &sites)?;
        negative_evidence.extend(transport.negative_evidence.iter().cloned());
        uncertainty.extend(transport.uncertainty.iter().cloned());
        if request.stop_on_qualified
            && transport.disposition == FederatedMechanismTransportDisposition::Qualified
        {
            stop_reason = FederatedMechanismTransportCampaignStopReason::Qualified;
            break;
        }
        if request.stop_on_negative
            && transport.disposition == FederatedMechanismTransportDisposition::Negative
        {
            stop_reason = FederatedMechanismTransportCampaignStopReason::Negative;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining == 0 {
            stop_reason = FederatedMechanismTransportCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut eligible = request
            .actions
            .iter()
            .filter(|action| {
                !completed.contains(&action.action_id)
                    && !failed.contains(&action.action_id)
                    && action.feasibility_milli > 0
                    && u64::from(action.cost_units) <= remaining
            })
            .collect::<Vec<_>>();
        eligible.sort_by(|left, right| {
            action_score(right, &transport)
                .cmp(&action_score(left, &transport))
                .then_with(|| left.action_id.cmp(&right.action_id))
        });
        if eligible.is_empty() {
            stop_reason = if request.actions.iter().any(|action| {
                !completed.contains(&action.action_id)
                    && !failed.contains(&action.action_id)
                    && action.feasibility_milli > 0
                    && u64::from(action.cost_units) > remaining
            }) {
                FederatedMechanismTransportCampaignStopReason::BudgetExhausted
            } else {
                FederatedMechanismTransportCampaignStopReason::NoEligibleActions
            };
            break;
        }
        let before_budget = remaining;
        let mut selected = Vec::new();
        let mut selected_cost = 0_u64;
        for action in eligible {
            let cost = u64::from(action.cost_units);
            if selected.is_empty() || selected_cost.saturating_add(cost) <= remaining {
                selected_cost = selected_cost.saturating_add(cost);
                selected.push(action);
            }
            if selected.len() >= 4 {
                break;
            }
        }
        let mut returned_site_order = Vec::new();
        let mut failed_action_order = Vec::new();
        let mut round_retries = 0_u32;
        let mut progress = false;
        for action in selected.iter().copied() {
            let mut returned = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_action(action, &request.transport, attempt) {
                    Ok(site) => {
                        validate_returned_site(&site, action, &request.transport, &sites)?;
                        returned = Some(site);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(FederatedMechanismTransportCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            round_retries = round_retries.saturating_add(1);
                            continue;
                        }
                        failed_action_order.push(action.action_id.clone());
                        failed.insert(action.action_id.clone());
                        break;
                    }
                }
            }
            if let Some(site) = returned {
                returned_site_order.push(site.site_id.clone());
                sites.push(site);
                completed.insert(action.action_id.clone());
                progress = true;
            }
        }
        returned_site_order.sort();
        failed_action_order.sort();
        budget_spent = budget_spent.saturating_add(selected_cost);
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated = analyze_federated_mechanism_transport(&request.transport, &sites)?;
        negative_evidence.extend(updated.negative_evidence.iter().cloned());
        uncertainty.extend(updated.uncertainty.iter().cloned());
        rounds.push(FederatedMechanismTransportCampaignRound {
            round: round_number,
            action_order: selected
                .iter()
                .map(|action| action.action_id.clone())
                .collect(),
            returned_site_order,
            failed_action_order,
            transport_disposition: updated.disposition,
            cost_units: selected_cost.min(u64::from(u32::MAX)) as u32,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: round_retries,
        });
        if !progress {
            stop_reason = FederatedMechanismTransportCampaignStopReason::ExecutorFailed;
            break;
        }
    }
    let final_transport = analyze_federated_mechanism_transport(&request.transport, &sites)?;
    if stop_reason == FederatedMechanismTransportCampaignStopReason::MaxRounds
        && rounds.len() < usize::from(request.max_rounds)
    {
        stop_reason = match final_transport.disposition {
            FederatedMechanismTransportDisposition::Qualified => {
                FederatedMechanismTransportCampaignStopReason::Qualified
            }
            FederatedMechanismTransportDisposition::Negative => {
                FederatedMechanismTransportCampaignStopReason::Negative
            }
            FederatedMechanismTransportDisposition::Heterogeneous => {
                FederatedMechanismTransportCampaignStopReason::Heterogeneous
            }
            _ => FederatedMechanismTransportCampaignStopReason::NoProgress,
        };
    }
    let campaign_disposition = disposition(final_transport.disposition);
    let next_step = match stop_reason {
        FederatedMechanismTransportCampaignStopReason::Qualified => {
            "review the cross-site transport analysis and independently reproduce the target-model effect".into()
        }
        FederatedMechanismTransportCampaignStopReason::Negative => {
            "preserve the negative transport result and design a falsification or boundary-condition follow-up".into()
        }
        FederatedMechanismTransportCampaignStopReason::Heterogeneous => {
            "stratify the mechanism by model/site signature and resolve the retained heterogeneity".into()
        }
        FederatedMechanismTransportCampaignStopReason::BudgetExhausted => {
            "allocate another bounded aggregate-only federation budget".into()
        }
        FederatedMechanismTransportCampaignStopReason::NoEligibleActions => {
            "supply a new aggregate-only site or replication action".into()
        }
        FederatedMechanismTransportCampaignStopReason::ExecutorFailed => {
            "repair the institution-local aggregate worker before retrying".into()
        }
        FederatedMechanismTransportCampaignStopReason::NoProgress => {
            "inspect unresolved site coverage and add an orthogonal aggregate measurement".into()
        }
        FederatedMechanismTransportCampaignStopReason::MaxRounds => {
            "resume with the returned completed and failed action partitions".into()
        }
    };
    let mut output = FederatedMechanismTransportCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.transport.objective.clone(),
        rounds,
        sites,
        completed_action_order: completed.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_transport,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition: campaign_disposition,
        stop_reason,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-transport-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedMechanismTransportCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_federated_mechanism_transport_campaign_dry_run(
    request: &FederatedMechanismTransportCampaignRequest,
) -> Result<FederatedMechanismTransportCampaign, FederatedMechanismTransportCampaignError> {
    let mut executor = DryRunFederatedMechanismTransportExecutor;
    execute_federated_mechanism_transport_campaign(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> FederatedMechanismTransportCampaignRequest {
        let transport = FederatedMechanismTransportRequest {
            objective: "transport glioma invasion mechanism".into(),
            mechanism_id: "invasion".into(),
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![0, 0],
            min_sites: 2,
            min_replicates_per_site: 1,
            min_quality_milli: 500,
            similarity_scale_milli: 1_000,
            effect_threshold_milli: 100,
            min_signal_to_noise_milli: 1,
            max_heterogeneity_milli: 800,
            max_site_spread_milli: 1_000,
            max_leave_one_out_shift_milli: 1_000,
            require_target_model: true,
        };
        let sites = vec![
            FederatedMechanismSite {
                site_id: "site-a".into(),
                study_id: "study-a".into(),
                mechanism_id: "invasion".into(),
                model_system: GliomaModelSystem::Organoid,
                effect_milli: 500,
                uncertainty_milli: 100,
                quality_milli: 900,
                replicate_count: 2,
                population_signature: vec![0, 0],
                artifact: artifact("a"),
            },
            FederatedMechanismSite {
                site_id: "site-b".into(),
                study_id: "study-b".into(),
                mechanism_id: "invasion".into(),
                model_system: GliomaModelSystem::Organoid,
                effect_milli: 520,
                uncertainty_milli: 100,
                quality_milli: 900,
                replicate_count: 2,
                population_signature: vec![0, 0],
                artifact: artifact("b"),
            },
        ];
        FederatedMechanismTransportCampaignRequest {
            transport,
            initial_sites: sites,
            actions: vec![FederatedMechanismTransportAction {
                action_id: "replicate-site-c".into(),
                target_site_id: Some("site-c".into()),
                model_system: GliomaModelSystem::Organoid,
                population_signature: vec![0, 0],
                cost_units: 1,
                expected_information_milli: 900,
                expected_effect_milli: 510,
                expected_heterogeneity_reduction_milli: 500,
                feasibility_milli: 900,
                risk_milli: 20,
                requested_replicates: 2,
            }],
            budget_units: 2,
            max_rounds: 2,
            max_retries: 1,
            stop_on_qualified: false,
            stop_on_negative: false,
        }
    }

    #[test]
    fn transport_campaign_executes_aggregate_follow_up_and_replays() {
        let result = execute_federated_mechanism_transport_campaign_dry_run(&request()).unwrap();
        assert_eq!(result.feature_id, FEATURE_ID);
        assert_eq!(result.rounds.len(), 1);
        assert_eq!(result.sites.len(), 3);
        assert_eq!(result.completed_action_order, vec!["replicate-site-c"]);
        result.validate().unwrap();
    }

    #[test]
    fn transport_campaign_rejects_human_or_remote_aggregate_artifacts() {
        let mut request = request();
        request.initial_sites[0].artifact.contains_human_data = true;
        assert!(matches!(
            execute_federated_mechanism_transport_campaign_dry_run(&request),
            Err(FederatedMechanismTransportCampaignError::Transport(_))
        ));
    }
}
