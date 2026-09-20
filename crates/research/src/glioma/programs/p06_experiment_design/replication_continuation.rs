//! Observation-driven continuation control for multi-site preclinical glioma replication.
//!
//! P06-F26 plans a replication topology from local site summaries. This feature closes the loop:
//! after each bounded wave, it reuses the topology calculation, applies an explicit quality gate,
//! evaluates directional and negative stopping boundaries, and emits the next site-level wave for
//! a caller-owned protocol executor. It never invents observations, silently promotes a weak site,
//! or makes a clinical decision.

use super::replication_plan::{
    plan_glioma_replication, ReplicationObservation, ReplicationPlan, ReplicationPlanDisposition,
    ReplicationPlanError, ReplicationPlanRequest, ReplicationSiteAction,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationContinuation1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_ROUNDS: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationContinuationObservation {
    pub round: u32,
    pub quality_milli: u16,
    pub observation: ReplicationObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationContinuationRequest {
    pub plan_request: ReplicationPlanRequest,
    pub current_round: u32,
    pub max_rounds: u32,
    pub minimum_quality_milli: u16,
    pub negative_effect_threshold_milli: u16,
    pub observations: Vec<ReplicationContinuationObservation>,
    pub previous_plan: Option<ReplicationPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationContinuationAction {
    ContinueReplication,
    AddIndependentSite,
    HoldForQuality,
    HoldHeterogeneity,
    StopQualified,
    StopNegative,
    StopRiskBlocked,
    StopBudgetBlocked,
    StopUnresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationContinuationDisposition {
    Continue,
    Qualified,
    Negative,
    Heterogeneous,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationContinuationSiteAction {
    pub site_id: String,
    pub action: ReplicationContinuationAction,
    pub current_replicates: u32,
    pub planned_new_replicates: u32,
    pub projected_cost_units: u64,
    pub quality_milli: u16,
    pub risk_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationContinuationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub current_round: u32,
    pub next_round: u32,
    pub current_topology: ReplicationPlan,
    pub site_order: Vec<String>,
    pub actions: Vec<ReplicationContinuationSiteAction>,
    pub pooled_effect_milli: i32,
    pub heterogeneity_milli: u16,
    pub power_proxy_milli: u16,
    pub quality_floor_milli: u16,
    pub target_met: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReplicationContinuationDisposition,
    pub next_action: ReplicationContinuationAction,
    pub stop_conditions: Vec<String>,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationContinuationError {
    #[error("replication continuation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication topology failed: {0}")]
    Topology(#[from] ReplicationPlanError),
    #[error("replication continuation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication continuation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &ReplicationContinuationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "endpoint": plan.endpoint,
        "current_round": plan.current_round,
        "next_round": plan.next_round,
        "current_topology": plan.current_topology,
        "site_order": plan.site_order,
        "actions": plan.actions,
        "pooled_effect_milli": plan.pooled_effect_milli,
        "heterogeneity_milli": plan.heterogeneity_milli,
        "power_proxy_milli": plan.power_proxy_milli,
        "quality_floor_milli": plan.quality_floor_milli,
        "target_met": plan.target_met,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
        "next_action": plan.next_action,
        "stop_conditions": plan.stop_conditions,
        "boundary": plan.boundary,
    })
}

fn validate_request(
    request: &ReplicationContinuationRequest,
) -> Result<(), ReplicationContinuationError> {
    if request.current_round == 0
        || request.current_round > request.max_rounds
        || request.max_rounds > MAX_ROUNDS
        || request.minimum_quality_milli > 1_000
        || request.negative_effect_threshold_milli > 1_000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(ReplicationContinuationError::InvalidRequest(
            "round, quality, negative-boundary, and bounded observations are required".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for item in &request.observations {
        if item.round == 0
            || item.round > request.current_round
            || item.quality_milli > 1_000
            || !keys.insert((
                item.round,
                item.observation.site_id.clone(),
                item.observation.arm_id.clone(),
            ))
        {
            return Err(ReplicationContinuationError::InvalidRequest(
                "observation rounds, quality, and round/site/arm uniqueness are invalid".into(),
            ));
        }
    }
    if let Some(previous) = &request.previous_plan {
        previous.validate().map_err(|error| {
            ReplicationContinuationError::InvalidRequest(format!(
                "previous replication plan is invalid: {error}"
            ))
        })?;
    }
    Ok(())
}

fn validate_output(plan: &ReplicationContinuationPlan) -> Result<(), ReplicationContinuationError> {
    if plan.feature_id != FEATURE_ID
        || plan.output_schema != OUTPUT_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.endpoint.trim().is_empty()
        || plan.current_round == 0
        || plan.next_round < plan.current_round
        || plan.next_round > MAX_ROUNDS
        || !canonical(&plan.site_order)
        || plan.actions.len() != plan.site_order.len()
        || plan.quality_floor_milli > 1_000
        || plan.heterogeneity_milli > 1_000
        || plan.power_proxy_milli > 1_000
        || plan
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        || plan.uncertainty.iter().any(|item| item.trim().is_empty())
        || plan.stop_conditions.is_empty()
        || plan
            .stop_conditions
            .iter()
            .any(|item| item.trim().is_empty())
        || plan.boundary != PRECLINICAL_BOUNDARY
        || plan.current_topology.validate().is_err()
        || plan.actions.iter().any(|action| {
            action.site_id.trim().is_empty()
                || action.current_replicates == 0
                || action.projected_cost_units == 0 && action.planned_new_replicates > 0
                || action.quality_milli > 1_000
                || action.risk_milli > 1_000
                || action.rationale.trim().is_empty()
        })
    {
        return Err(ReplicationContinuationError::InvalidOutput(
            "identity, topology, ordering, quality, action, or boundary fields are invalid".into(),
        ));
    }
    let action_ids = plan
        .actions
        .iter()
        .map(|action| action.site_id.clone())
        .collect::<Vec<_>>();
    if action_ids != plan.site_order {
        return Err(ReplicationContinuationError::InvalidOutput(
            "site actions do not reconcile with the topology order".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(plan))
        .map_err(|error| ReplicationContinuationError::Digest(error.to_string()))?;
    if expected != plan.digest {
        return Err(ReplicationContinuationError::InvalidOutput(
            "continuation digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ReplicationContinuationPlan {
    pub fn validate(&self) -> Result<(), ReplicationContinuationError> {
        validate_output(self)
    }
}

fn action_for_site(
    site_action: ReplicationSiteAction,
    quality_milli: u16,
    minimum_quality_milli: u16,
    planned_new_replicates: u32,
) -> ReplicationContinuationAction {
    if quality_milli < minimum_quality_milli {
        ReplicationContinuationAction::HoldForQuality
    } else {
        match site_action {
            ReplicationSiteAction::RiskBlocked => ReplicationContinuationAction::StopRiskBlocked,
            ReplicationSiteAction::BudgetBlocked => {
                ReplicationContinuationAction::StopBudgetBlocked
            }
            ReplicationSiteAction::Heterogeneous => {
                ReplicationContinuationAction::HoldHeterogeneity
            }
            ReplicationSiteAction::InsufficientPair => {
                ReplicationContinuationAction::AddIndependentSite
            }
            ReplicationSiteAction::Replicate if planned_new_replicates > 0 => {
                ReplicationContinuationAction::ContinueReplication
            }
            ReplicationSiteAction::HoldUnderpowered => {
                ReplicationContinuationAction::ContinueReplication
            }
            ReplicationSiteAction::Replicate => ReplicationContinuationAction::ContinueReplication,
        }
    }
}

/// Replan the next local replication wave from observations already returned by prior rounds.
pub fn plan_glioma_replication_continuation(
    request: &ReplicationContinuationRequest,
) -> Result<ReplicationContinuationPlan, ReplicationContinuationError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        left.observation
            .site_id
            .cmp(&right.observation.site_id)
            .then(left.observation.arm_id.cmp(&right.observation.arm_id))
            .then(left.round.cmp(&right.round))
    });
    let mut latest_by_arm = BTreeMap::new();
    for item in observations {
        latest_by_arm.insert(
            (
                item.observation.site_id.clone(),
                item.observation.arm_id.clone(),
            ),
            item,
        );
    }
    let observations = latest_by_arm.into_values().collect::<Vec<_>>();
    let topology_observations = observations
        .iter()
        .map(|item| item.observation.clone())
        .collect::<Vec<_>>();
    let current_topology = plan_glioma_replication(&request.plan_request, &topology_observations)?;
    let quality_floor = observations
        .iter()
        .map(|item| item.quality_milli)
        .min()
        .unwrap_or(0);
    let low_quality = quality_floor < request.minimum_quality_milli;
    let target_sign = request.plan_request.target_effect_milli.signum();
    let target_met = !low_quality
        && current_topology.power_proxy_milli >= request.plan_request.power_target_milli
        && current_topology.pooled_effect_milli.signum() == target_sign
        && current_topology.pooled_effect_milli.unsigned_abs()
            >= request.plan_request.target_effect_milli.unsigned_abs();
    let negative_boundary = !low_quality
        && current_topology.power_proxy_milli >= request.plan_request.power_target_milli
        && (current_topology.pooled_effect_milli.signum() != target_sign
            || current_topology.pooled_effect_milli.unsigned_abs()
                < u32::from(request.negative_effect_threshold_milli));
    let round_limit = request.current_round >= request.max_rounds;
    let mut uncertainty = BTreeSet::from([
        "quality is an input gate, not proof of biological validity".to_string(),
        "power_proxy is a bounded planning approximation and requires independent replication"
            .to_string(),
        "all effects remain preclinical, local, and caller-owned; no clinical decision is emitted"
            .to_string(),
    ]);
    let mut negative_evidence = BTreeSet::new();
    if low_quality {
        uncertainty.insert(format!(
            "quality-floor:{}<{}",
            quality_floor, request.minimum_quality_milli
        ));
        negative_evidence.insert("one-or-more-observation-quality-gates-failed".into());
    }
    if current_topology.heterogeneity_milli > request.plan_request.max_site_heterogeneity_milli {
        negative_evidence.insert(format!(
            "heterogeneity:{}>{}",
            current_topology.heterogeneity_milli, request.plan_request.max_site_heterogeneity_milli
        ));
    }
    if negative_boundary {
        negative_evidence.insert(format!(
            "negative-boundary:{}<{}",
            current_topology.pooled_effect_milli.unsigned_abs(),
            request.negative_effect_threshold_milli
        ));
    }
    if round_limit {
        uncertainty.insert("maximum continuation round reached".into());
    }
    let mut actions = Vec::new();
    for site_plan in &current_topology.plans {
        let site_quality = observations
            .iter()
            .filter(|item| item.observation.site_id == site_plan.site_id)
            .map(|item| item.quality_milli)
            .min()
            .unwrap_or(0);
        let planned_new = site_plan
            .planned_replicates
            .saturating_sub(site_plan.current_replicates);
        let action = action_for_site(
            site_plan.action,
            site_quality,
            request.minimum_quality_milli,
            planned_new,
        );
        actions.push(ReplicationContinuationSiteAction {
            site_id: site_plan.site_id.clone(),
            action,
            current_replicates: site_plan.current_replicates,
            planned_new_replicates: if matches!(
                action,
                ReplicationContinuationAction::ContinueReplication
            ) {
                planned_new
            } else {
                0
            },
            projected_cost_units: if matches!(
                action,
                ReplicationContinuationAction::ContinueReplication
            ) {
                site_plan
                    .projected_cost_units
                    .saturating_sub(u64::from(site_plan.current_replicates))
            } else {
                0
            },
            quality_milli: site_quality,
            risk_milli: site_plan.risk_milli,
            rationale: format!(
                "quality {site_quality}, site action {:?}, current replicates {}, planned new replicates {planned_new}",
                site_plan.action, site_plan.current_replicates
            ),
        });
    }
    let risk_blocked = current_topology
        .plans
        .iter()
        .any(|site| site.action == ReplicationSiteAction::RiskBlocked);
    let budget_blocked = current_topology
        .plans
        .iter()
        .any(|site| site.action == ReplicationSiteAction::BudgetBlocked);
    let disposition = if low_quality {
        ReplicationContinuationDisposition::Unresolved
    } else if target_met {
        ReplicationContinuationDisposition::Qualified
    } else if negative_boundary {
        ReplicationContinuationDisposition::Negative
    } else if current_topology.disposition == ReplicationPlanDisposition::Heterogeneous {
        ReplicationContinuationDisposition::Heterogeneous
    } else if risk_blocked || budget_blocked {
        ReplicationContinuationDisposition::Blocked
    } else if round_limit || current_topology.plans.is_empty() {
        ReplicationContinuationDisposition::Unresolved
    } else {
        ReplicationContinuationDisposition::Continue
    };
    let next_action = match disposition {
        ReplicationContinuationDisposition::Continue => {
            if current_topology.plans.len() < request.plan_request.min_sites {
                ReplicationContinuationAction::AddIndependentSite
            } else {
                ReplicationContinuationAction::ContinueReplication
            }
        }
        ReplicationContinuationDisposition::Qualified => {
            ReplicationContinuationAction::StopQualified
        }
        ReplicationContinuationDisposition::Negative => ReplicationContinuationAction::StopNegative,
        ReplicationContinuationDisposition::Heterogeneous => {
            ReplicationContinuationAction::HoldHeterogeneity
        }
        ReplicationContinuationDisposition::Blocked => {
            if risk_blocked {
                ReplicationContinuationAction::StopRiskBlocked
            } else {
                ReplicationContinuationAction::StopBudgetBlocked
            }
        }
        ReplicationContinuationDisposition::Unresolved => {
            if low_quality {
                ReplicationContinuationAction::HoldForQuality
            } else {
                ReplicationContinuationAction::StopUnresolved
            }
        }
    };
    let next_round = if matches!(
        next_action,
        ReplicationContinuationAction::ContinueReplication
            | ReplicationContinuationAction::AddIndependentSite
    ) {
        request
            .current_round
            .saturating_add(1)
            .min(request.max_rounds)
    } else {
        request.current_round
    };
    let next_action_text = match next_action {
        ReplicationContinuationAction::ContinueReplication => {
            "submit the bounded next replication wave to a caller-owned local protocol executor"
        }
        ReplicationContinuationAction::AddIndependentSite => {
            "add an independent preclinical site before interpreting the pooled effect"
        }
        ReplicationContinuationAction::HoldForQuality => {
            "repair or adjudicate low-quality local observations before replanning"
        }
        ReplicationContinuationAction::HoldHeterogeneity => {
            "inspect site-specific protocol, model-system, and batch causes before pooling"
        }
        ReplicationContinuationAction::StopQualified => {
            "hold the qualified result for independent review and signed release evidence"
        }
        ReplicationContinuationAction::StopNegative => {
            "publish the negative or null replication result and stop spending replication budget"
        }
        ReplicationContinuationAction::StopRiskBlocked => {
            "obtain bounded local risk review before any additional replication"
        }
        ReplicationContinuationAction::StopBudgetBlocked => {
            "increase the declared replication budget or reduce local assay cost"
        }
        ReplicationContinuationAction::StopUnresolved => {
            "resolve missing paired sites, observations, or continuation gates before proceeding"
        }
    }
    .to_string();
    let mut plan = ReplicationContinuationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.plan_request.objective.clone(),
        model_system: request.plan_request.model_system,
        endpoint: request.plan_request.endpoint.clone(),
        current_round: request.current_round,
        next_round,
        site_order: current_topology.site_order.clone(),
        current_topology,
        actions,
        pooled_effect_milli: 0,
        heterogeneity_milli: 0,
        power_proxy_milli: 0,
        quality_floor_milli: quality_floor,
        target_met,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        stop_conditions: vec![
            "never promote a planning proxy into biological efficacy".into(),
            "never dispatch assays, instruments, federation, or clinical actions from this plan"
                .into(),
            "stop when quality, heterogeneity, risk, budget, or paired-site gates are unresolved"
                .into(),
        ],
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-continuation"),
    };
    plan.pooled_effect_milli = plan.current_topology.pooled_effect_milli;
    plan.heterogeneity_milli = plan.current_topology.heterogeneity_milli;
    plan.power_proxy_milli = plan.current_topology.power_proxy_milli;
    plan.next_action = next_action;
    plan.uncertainty.push(next_action_text);
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ReplicationContinuationError::Digest(error.to_string()))?;
    validate_output(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;
    use bioprism_ids::ContentHash;

    fn observation(
        site_id: &str,
        arm_id: &str,
        mean: i32,
        quality: u16,
    ) -> ReplicationContinuationObservation {
        ReplicationContinuationObservation {
            round: 1,
            quality_milli: quality,
            observation: ReplicationObservation {
                site_id: site_id.into(),
                arm_id: arm_id.into(),
                label: format!("{site_id}-{arm_id}"),
                artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{site_id}-{arm_id}"),
                    content_hash: ContentHash::of_bytes(format!("{site_id}-{arm_id}").as_bytes()),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                model_system: GliomaModelSystem::Organoid,
                mean_response_milli: mean,
                variance_milli2: 100,
                observations: 3,
                cost_units_per_replicate: 2,
                risk_milli: 100,
            },
        }
    }

    fn request() -> ReplicationContinuationRequest {
        ReplicationContinuationRequest {
            plan_request: ReplicationPlanRequest {
                objective: "continue organoid invasion replication".into(),
                model_system: GliomaModelSystem::Organoid,
                endpoint: "invasion".into(),
                control_arm_id: "control".into(),
                treatment_arm_id: "treated".into(),
                target_effect_milli: 200,
                alpha_total_milli: 50,
                power_target_milli: 600,
                min_sites: 2,
                max_sites: 4,
                min_replicates_per_site: 2,
                max_replicates_per_site: 20,
                budget_units: 200,
                max_total_replicates: 100,
                max_site_heterogeneity_milli: 200,
                risk_ceiling_milli: 500,
            },
            current_round: 1,
            max_rounds: 4,
            minimum_quality_milli: 700,
            negative_effect_threshold_milli: 40,
            observations: vec![
                observation("site-a", "control", 100, 900),
                observation("site-a", "treated", 240, 900),
                observation("site-b", "control", 110, 900),
                observation("site-b", "treated", 250, 900),
            ],
            previous_plan: None,
        }
    }

    #[test]
    fn continuation_is_replay_stable_and_requests_more_replication() {
        let request = request();
        let left = plan_glioma_replication_continuation(&request).unwrap();
        let right = plan_glioma_replication_continuation(&request).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            left.disposition,
            ReplicationContinuationDisposition::Continue
        );
        assert_eq!(
            left.next_action,
            ReplicationContinuationAction::ContinueReplication
        );
        assert_eq!(left.next_round, 2);
        assert!(left
            .actions
            .iter()
            .any(|action| action.planned_new_replicates > 0));
        left.validate().unwrap();
    }

    #[test]
    fn low_quality_observation_holds_without_promoting_effect() {
        let mut request = request();
        request.observations[0].quality_milli = 400;
        let output = plan_glioma_replication_continuation(&request).unwrap();
        assert_eq!(
            output.disposition,
            ReplicationContinuationDisposition::Unresolved
        );
        assert_eq!(
            output.next_action,
            ReplicationContinuationAction::HoldForQuality
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("quality")));
    }
}
