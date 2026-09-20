//! Execute a selected replication-closure frontier for preclinical glioma research.
//!
//! The closure frontier is a scientific ranking, not an authority grant. This feature is the
//! explicit execution seam: it admits only a selected candidate whose route names the bounded
//! replication campaign, then delegates the actual work to the institution-owned executor. A
//! methods-review hold, a blocked upstream gate, or a frontier with no runnable selection cannot
//! silently become a campaign.

use super::campaign::{
    execute_glioma_replication_campaign, GliomaReplicationCampaign,
    GliomaReplicationCampaignDisposition, GliomaReplicationCampaignError,
    GliomaReplicationCampaignExecutor, GliomaReplicationCampaignRequest,
    GliomaReplicationCampaignStopReason,
};
use super::replication_closure_frontier::{
    ReplicationClosureDisposition, ReplicationClosureFrontier,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationClosureExecution1@1";
pub const EXECUTION_ROUTE: &str = "glioma_validation_replication_campaign_execute";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureExecutionRequest {
    pub frontier: ReplicationClosureFrontier,
    pub campaign: GliomaReplicationCampaignRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationClosureExecutionDisposition {
    HeldByFrontier,
    Executed,
    Qualified,
    Negative,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClosureExecutionRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub frontier_digest: ContentHash,
    pub source_frontier_disposition: ReplicationClosureDisposition,
    pub selected_action_order: Vec<String>,
    pub executable_action_order: Vec<String>,
    pub campaign: Option<GliomaReplicationCampaign>,
    pub disposition: ReplicationClosureExecutionDisposition,
    pub stop_reason: Option<GliomaReplicationCampaignStopReason>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationClosureExecutionError {
    #[error("replication closure request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication closure campaign failed: {0}")]
    Campaign(#[from] GliomaReplicationCampaignError),
    #[error("replication closure output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication closure digest failed: {0}")]
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

fn digest_input(output: &ReplicationClosureExecutionRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "frontier_digest": output.frontier_digest,
        "source_frontier_disposition": output.source_frontier_disposition,
        "selected_action_order": output.selected_action_order,
        "executable_action_order": output.executable_action_order,
        "campaign": output.campaign,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn validate_request(
    request: &ReplicationClosureExecutionRequest,
) -> Result<Vec<String>, ReplicationClosureExecutionError> {
    request
        .frontier
        .validate()
        .map_err(|error| ReplicationClosureExecutionError::InvalidRequest(error.to_string()))?;
    if request.campaign.objective != request.frontier.objective
        || request.campaign.model_system != request.frontier.model_system
        || request.campaign.target_model_system != request.frontier.model_system
    {
        return Err(ReplicationClosureExecutionError::InvalidRequest(
            "campaign objective and model systems must match the closure frontier".into(),
        ));
    }
    let selected = request
        .frontier
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let executable = request
        .frontier
        .scores
        .iter()
        .filter(|score| selected.contains(&score.action_id) && score.route == EXECUTION_ROUTE)
        .map(|score| score.action_id.clone())
        .collect::<Vec<_>>();
    Ok(executable)
}

fn finish(
    request: &ReplicationClosureExecutionRequest,
    executable_action_order: Vec<String>,
    campaign: Option<GliomaReplicationCampaign>,
    disposition: ReplicationClosureExecutionDisposition,
    stop_reason: Option<GliomaReplicationCampaignStopReason>,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    next_action: &str,
) -> Result<ReplicationClosureExecutionRun, ReplicationClosureExecutionError> {
    let mut output = ReplicationClosureExecutionRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.frontier.objective.clone(),
        model_system: request.frontier.model_system,
        frontier_digest: request.frontier.digest.clone(),
        source_frontier_disposition: request.frontier.disposition,
        selected_action_order: request.frontier.selected_order.clone(),
        executable_action_order,
        campaign,
        disposition,
        stop_reason,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-closure-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReplicationClosureExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl ReplicationClosureExecutionRun {
    pub fn validate(&self) -> Result<(), ReplicationClosureExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.selected_action_order)
            || !canonical(&self.executable_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .executable_action_order
                .iter()
                .any(|id| self.selected_action_order.binary_search(id).is_err())
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.next_action.trim().is_empty()
            || self
                .campaign
                .as_ref()
                .is_some_and(|campaign| campaign.validate().is_err())
        {
            return Err(ReplicationClosureExecutionError::InvalidOutput(
                "identity, ordering, action binding, boundary, campaign, or next-action invariant failed".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReplicationClosureExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReplicationClosureExecutionError::InvalidOutput(
                "replication closure execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute the selected replication-closure action through the bounded campaign executor.
pub fn execute_glioma_replication_closure<E: GliomaReplicationCampaignExecutor>(
    request: &ReplicationClosureExecutionRequest,
    executor: &mut E,
) -> Result<ReplicationClosureExecutionRun, ReplicationClosureExecutionError> {
    let executable_action_order = validate_request(request)?;
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if executable_action_order.is_empty()
        || matches!(
            request.frontier.disposition,
            ReplicationClosureDisposition::Blocked
                | ReplicationClosureDisposition::NoRunnableActions
        )
    {
        negative_evidence.push("closure-frontier-held-before-campaign-dispatch".into());
        uncertainty.push(
            "the closure frontier did not select an executable campaign route with permission to proceed".into(),
        );
        return finish(
            request,
            executable_action_order,
            None,
            ReplicationClosureExecutionDisposition::HeldByFrontier,
            None,
            negative_evidence,
            uncertainty,
            "revise the bounded frontier or complete methods review before campaign execution",
        );
    }
    let campaign = execute_glioma_replication_campaign(&request.campaign, executor)?;
    negative_evidence.extend(campaign.negative_evidence.clone());
    uncertainty.extend(campaign.uncertainty.clone());
    let (disposition, next_action) = match campaign.disposition {
        GliomaReplicationCampaignDisposition::Qualified => (
            ReplicationClosureExecutionDisposition::Qualified,
            "hold the qualified closure campaign for independent methods review and release evidence",
        ),
        GliomaReplicationCampaignDisposition::Negative => (
            ReplicationClosureExecutionDisposition::Negative,
            "publish the negative closure result and preserve its estimand and boundary conditions",
        ),
        GliomaReplicationCampaignDisposition::Partial => (
            ReplicationClosureExecutionDisposition::Partial,
            "continue with a new bounded closure frontier using the returned campaign partitions",
        ),
        GliomaReplicationCampaignDisposition::Unresolved => (
            ReplicationClosureExecutionDisposition::Unresolved,
            "resolve missing or contradictory replication evidence before another execution wave",
        ),
        GliomaReplicationCampaignDisposition::Failed | GliomaReplicationCampaignDisposition::Blocked => (
            ReplicationClosureExecutionDisposition::Blocked,
            "repair the institution-local executor or policy boundary before retrying",
        ),
    };
    finish(
        request,
        executable_action_order,
        Some(campaign.clone()),
        disposition,
        Some(campaign.stop_reason),
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::super::validation_replication_campaign::ValidationReplicationCampaignDisposition;
    use super::*;

    fn held_frontier() -> ReplicationClosureFrontier {
        let mut frontier = ReplicationClosureFrontier {
            feature_id: "GAF-GLIOMA-P10-F27".into(),
            output_schema: "GliomaReplicationClosureFrontier1@1".into(),
            objective: "replicate organoid invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            source_replication_digest: ContentHash::of_bytes(b"replication"),
            source_disposition: ValidationReplicationCampaignDisposition::BlockedByValidation,
            source_stop_reason: None,
            candidate_order: vec!["confirm".into()],
            scores: vec![super::super::replication_closure_frontier::ReplicationClosureScore {
                action_id: "confirm".into(),
                route: EXECUTION_ROUTE.into(),
                model_system: GliomaModelSystem::Organoid,
                target: super::super::replication_closure_frontier::ReplicationClosureTarget::ConfirmNegativeResult,
                utility_milli: 700,
                evidence_gap_milli: 900,
                risk_adjusted_information_milli: 700,
                cost_units: 1,
                decision: "blocked_by_replication_state".into(),
            }],
            selected_order: Vec::new(),
            deferred_order: Vec::new(),
            blocked_order: vec!["confirm".into()],
            negative_evidence: vec!["upstream-blocked".into()],
            uncertainty: vec!["awaiting validation".into()],
            disposition: ReplicationClosureDisposition::Blocked,
            next_operator_action: "complete validation".into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let input = serde_json::json!({
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
        frontier.digest = ContentHash::of_value(&input).unwrap();
        frontier
    }

    #[test]
    fn held_frontier_never_dispatches_replication_campaign() {
        let request = ReplicationClosureExecutionRequest {
            frontier: held_frontier(),
            campaign: GliomaReplicationCampaignRequest {
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
            },
        };
        let mut executor =
            super::super::campaign::DryRunGliomaReplicationCampaignExecutor::default();
        let output = execute_glioma_replication_closure(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ReplicationClosureExecutionDisposition::HeldByFrontier
        );
        assert!(output.campaign.is_none());
        output.validate().unwrap();
    }
}
