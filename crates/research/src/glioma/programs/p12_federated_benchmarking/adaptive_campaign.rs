//! Adaptive federated benchmark planning and aggregate-only execution.
//!
//! The site planner produces a conservative portfolio projection; the campaign executor
//! produces the observations that can confirm or contradict that projection. This feature closes
//! that loop without moving raw traces: it converts selected candidate sites into typed aggregate
//! actions, executes them through the existing institution-local seam, and reports projected versus
//! observed consensus as separate scientific states.

use super::campaign::{
    execute_federated_benchmark_campaign, FederatedBenchmarkAction, FederatedBenchmarkActionKind,
    FederatedBenchmarkCampaign, FederatedBenchmarkCampaignDisposition,
    FederatedBenchmarkCampaignError, FederatedBenchmarkCampaignExecutor,
    FederatedBenchmarkCampaignRequest,
};
use super::consensus::FederatedBenchmarkDisposition;
use super::site_planner::{
    plan_federated_benchmark_sites, FederatedBenchmarkSitePlan, FederatedBenchmarkSitePlannerError,
    FederatedBenchmarkSitePlannerRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedAdaptiveCampaign1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkAdaptiveCampaignRequest {
    pub planning: FederatedBenchmarkSitePlannerRequest,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
    pub require_aggregate_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkAdaptiveDisposition {
    Qualified,
    Negative,
    Heterogeneous,
    Partial,
    Blocked,
    NoAdmissiblePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkAdaptiveStopReason {
    ProjectedQualified,
    ObservedQualified,
    ObservedNegative,
    ObservedHeterogeneous,
    NoAdmissiblePlan,
    CampaignBlocked,
    CampaignFailed,
    CampaignUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkAdaptiveCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub site_plan: FederatedBenchmarkSitePlan,
    pub planned_site_order: Vec<String>,
    pub planned_action_order: Vec<String>,
    pub campaign: Option<FederatedBenchmarkCampaign>,
    pub projected_disposition: FederatedBenchmarkDisposition,
    pub observed_disposition: Option<FederatedBenchmarkCampaignDisposition>,
    pub simulation_only: bool,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkAdaptiveDisposition,
    pub stop_reason: FederatedBenchmarkAdaptiveStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkAdaptiveCampaignError {
    #[error("adaptive federated site planning failed: {0}")]
    Planning(#[from] FederatedBenchmarkSitePlannerError),
    #[error("adaptive federated campaign failed: {0}")]
    Campaign(#[from] FederatedBenchmarkCampaignError),
    #[error("adaptive federated boundary blocked: {0}")]
    Boundary(String),
    #[error("adaptive federated output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive federated digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedBenchmarkAdaptiveCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "site_plan": output.site_plan,
        "planned_site_order": output.planned_site_order,
        "planned_action_order": output.planned_action_order,
        "campaign": output.campaign,
        "projected_disposition": output.projected_disposition,
        "observed_disposition": output.observed_disposition,
        "simulation_only": output.simulation_only,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn validate_aggregate_site(
    site: &super::consensus::FederatedBenchmarkSite,
) -> Result<(), FederatedBenchmarkAdaptiveCampaignError> {
    site.artifact
        .validate()
        .map_err(|error| FederatedBenchmarkAdaptiveCampaignError::Boundary(error.to_string()))?;
    if !site.artifact.local_only
        || site.artifact.contains_human_data
        || site.artifact.contains_direct_identifiers
    {
        return Err(FederatedBenchmarkAdaptiveCampaignError::Boundary(
            "adaptive federation requires local-only, non-human, non-identifying aggregate artifacts".into(),
        ));
    }
    Ok(())
}

fn disposition(
    projected: FederatedBenchmarkDisposition,
    campaign: Option<&FederatedBenchmarkCampaign>,
) -> (
    FederatedBenchmarkAdaptiveDisposition,
    FederatedBenchmarkAdaptiveStopReason,
) {
    let Some(campaign) = campaign else {
        return (
            FederatedBenchmarkAdaptiveDisposition::NoAdmissiblePlan,
            FederatedBenchmarkAdaptiveStopReason::NoAdmissiblePlan,
        );
    };
    let mapped = match campaign.disposition {
        FederatedBenchmarkCampaignDisposition::Qualified => (
            FederatedBenchmarkAdaptiveDisposition::Qualified,
            FederatedBenchmarkAdaptiveStopReason::ObservedQualified,
        ),
        FederatedBenchmarkCampaignDisposition::Negative => (
            FederatedBenchmarkAdaptiveDisposition::Negative,
            FederatedBenchmarkAdaptiveStopReason::ObservedNegative,
        ),
        FederatedBenchmarkCampaignDisposition::Heterogeneous => (
            FederatedBenchmarkAdaptiveDisposition::Heterogeneous,
            FederatedBenchmarkAdaptiveStopReason::ObservedHeterogeneous,
        ),
        FederatedBenchmarkCampaignDisposition::Partial => (
            FederatedBenchmarkAdaptiveDisposition::Partial,
            FederatedBenchmarkAdaptiveStopReason::CampaignUnresolved,
        ),
        FederatedBenchmarkCampaignDisposition::BudgetBlocked => (
            FederatedBenchmarkAdaptiveDisposition::Blocked,
            FederatedBenchmarkAdaptiveStopReason::CampaignBlocked,
        ),
        FederatedBenchmarkCampaignDisposition::Failed => (
            FederatedBenchmarkAdaptiveDisposition::Blocked,
            FederatedBenchmarkAdaptiveStopReason::CampaignFailed,
        ),
        FederatedBenchmarkCampaignDisposition::Unresolved => (
            FederatedBenchmarkAdaptiveDisposition::Blocked,
            FederatedBenchmarkAdaptiveStopReason::CampaignUnresolved,
        ),
    };
    if projected == FederatedBenchmarkDisposition::Qualified
        && mapped.0 != FederatedBenchmarkAdaptiveDisposition::Qualified
    {
        (FederatedBenchmarkAdaptiveDisposition::Partial, mapped.1)
    } else {
        mapped
    }
}

impl FederatedBenchmarkAdaptiveCampaign {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkAdaptiveCampaignError> {
        self.site_plan.validate().map_err(|error| {
            FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(error.to_string())
        })?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.objective != self.site_plan.objective
            || !canonical(&self.planned_site_order)
            || !canonical(&self.planned_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.projected_disposition != self.site_plan.projected.disposition
            || self.campaign.is_none()
                && self.disposition != FederatedBenchmarkAdaptiveDisposition::NoAdmissiblePlan
            || self.campaign.is_some() && self.observed_disposition.is_none()
            || self.next_operator_action.trim().is_empty()
        {
            return Err(FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(
                "identity, plan binding, ordering, campaign presence, disposition, or operator-action invariant failed".into(),
            ));
        }
        if let Some(campaign) = &self.campaign {
            campaign.validate().map_err(|error| {
                FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(error.to_string())
            })?;
            if campaign.objective != self.objective {
                return Err(FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(
                    "campaign objective does not match site plan objective".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkAdaptiveCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(
                "adaptive federated campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run conservative federated site selection and execute the selected aggregate-only portfolio.
pub fn execute_federated_benchmark_adaptive_campaign<E: FederatedBenchmarkCampaignExecutor>(
    request: &FederatedBenchmarkAdaptiveCampaignRequest,
    executor: &mut E,
) -> Result<FederatedBenchmarkAdaptiveCampaign, FederatedBenchmarkAdaptiveCampaignError> {
    if request.max_rounds == 0 || request.max_retries > 6 || request.planning.budget_units == 0 {
        return Err(FederatedBenchmarkAdaptiveCampaignError::Boundary(
            "positive bounded rounds, retries, and planning budget are required".into(),
        ));
    }
    if request.require_aggregate_only {
        for site in &request.planning.current_sites {
            validate_aggregate_site(site)?;
        }
        for candidate in &request.planning.candidates {
            validate_aggregate_site(&super::consensus::FederatedBenchmarkSite {
                site_id: candidate.site_id.clone(),
                study_id: candidate.study_id.clone(),
                capability_id: request.planning.benchmark.capability_id.clone(),
                benchmark_world: request.planning.benchmark.benchmark_world.clone(),
                metric_name: request.planning.benchmark.metric_name.clone(),
                model_system: request.planning.benchmark.model_system,
                artifact: candidate.artifact.clone(),
                baseline_score_milli: candidate.baseline_score_milli,
                candidate_score_milli: candidate.expected_candidate_score_milli,
                uncertainty_milli: candidate.uncertainty_milli,
                replicate_count: candidate.replicate_count,
            })?;
        }
    }
    let site_plan = plan_federated_benchmark_sites(&request.planning)?;
    let candidates = request
        .planning
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut planned_action_order = Vec::new();
    let mut actions = Vec::new();
    for candidate_id in &site_plan.selected_order {
        let candidate = candidates.get(candidate_id).ok_or_else(|| {
            FederatedBenchmarkAdaptiveCampaignError::InvalidOutput(
                "site plan selected an unknown candidate".into(),
            )
        })?;
        let action_id = format!("add-federated-site:{candidate_id}");
        planned_action_order.push(action_id.clone());
        actions.push(FederatedBenchmarkAction {
            action_id,
            kind: FederatedBenchmarkActionKind::ExpandCoverage,
            target_site_id: Some(candidate.site_id.clone()),
            cost_units: candidate.cost_units,
            expected_information_milli: candidate
                .expected_candidate_score_milli
                .saturating_sub(candidate.baseline_score_milli),
            expected_effect_milli: candidate.expected_candidate_score_milli,
            feasibility_milli: 1_000_u16
                .saturating_sub(candidate.uncertainty_milli.min(1_000) as u16),
            risk_milli: candidate.privacy_risk_milli,
            requested_replicates: candidate.replicate_count,
        });
    }
    planned_action_order.sort();
    let (campaign, mut negative_evidence, mut uncertainty) = if actions.is_empty() {
        (
            None,
            site_plan.negative_evidence.clone(),
            site_plan.uncertainty.clone(),
        )
    } else {
        let campaign_request = FederatedBenchmarkCampaignRequest {
            benchmark: request.planning.benchmark.clone(),
            initial_sites: request.planning.current_sites.clone(),
            actions,
            budget_units: site_plan.selected_cost_units,
            max_rounds: request.max_rounds,
            max_retries: request.max_retries,
            stop_on_qualified: request.stop_on_qualified,
            stop_on_negative: request.stop_on_negative,
        };
        let campaign = execute_federated_benchmark_campaign(&campaign_request, executor)?;
        let mut negative = site_plan.negative_evidence.clone();
        negative.extend(campaign.negative_evidence.iter().cloned());
        let mut uncertain = site_plan.uncertainty.clone();
        uncertain.extend(campaign.uncertainty.iter().cloned());
        (Some(campaign), negative, uncertain)
    };
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let (disposition, stop_reason) =
        disposition(site_plan.projected.disposition, campaign.as_ref());
    let next_operator_action = match (campaign.as_ref(), disposition) {
        (None, FederatedBenchmarkAdaptiveDisposition::NoAdmissiblePlan) => {
            "revise budget, privacy, independence, or candidate coverage before federated dispatch".into()
        }
        (Some(campaign), FederatedBenchmarkAdaptiveDisposition::Qualified) => {
            format!("compare observed aggregate consensus with the conservative projection, then publish only the validated benchmark: {:?}", campaign.final_consensus.disposition)
        }
        (Some(_), FederatedBenchmarkAdaptiveDisposition::Negative) => {
            "retain the negative aggregate benchmark and inspect site-local failure modes before another round".into()
        }
        (Some(_), FederatedBenchmarkAdaptiveDisposition::Heterogeneous) => {
            "retain site-stratified aggregates and resolve heterogeneity before pooling or transport claims".into()
        }
        (Some(_), _) => "replan only from returned aggregate sites; projected consensus is not an observation".into(),
        (None, _) => "resolve the federated candidate and privacy gates before dispatch".into(),
    };
    let mut output = FederatedBenchmarkAdaptiveCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.planning.benchmark.objective.clone(),
        planned_site_order: site_plan.selected_order.clone(),
        planned_action_order,
        projected_disposition: site_plan.projected.disposition,
        observed_disposition: campaign.as_ref().map(|campaign| campaign.disposition),
        simulation_only: true,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition,
        stop_reason,
        site_plan,
        campaign,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-adaptive-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkAdaptiveCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_federated_benchmark_adaptive_campaign_dry_run(
    request: &FederatedBenchmarkAdaptiveCampaignRequest,
) -> Result<FederatedBenchmarkAdaptiveCampaign, FederatedBenchmarkAdaptiveCampaignError> {
    let mut executor = super::campaign::DryRunFederatedBenchmarkCampaignExecutor;
    execute_federated_benchmark_adaptive_campaign(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::super::site_planner::{
        FederatedBenchmarkCandidate, FederatedBenchmarkSitePlannerRequest,
    };
    use super::*;

    // The public API test below uses the same deterministic fixture shape as the site planner's
    // unit suite; keeping the construction local prevents hidden mutable global state.
    fn request() -> FederatedBenchmarkAdaptiveCampaignRequest {
        let hash = ContentHash::of_bytes(b"adaptive-federated-campaign");
        let artifact = |id: &str| crate::glioma_engine::LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash.clone(),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let benchmark = super::super::consensus::FederatedBenchmarkRequest {
            objective: "validate invasion model across independent organoids".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: crate::glioma_engine::GliomaModelSystem::Organoid,
            minimum_sites: 3,
            minimum_replicates_per_site: 3,
            effect_threshold_milli: 80,
            max_i2_milli: 250,
            min_signal_to_noise_milli: 500,
            max_site_spread_milli: 160,
            max_leave_one_out_shift_milli: 100,
        };
        let current_benchmark = benchmark.clone();
        let current = |id: &str, score: u64| super::super::consensus::FederatedBenchmarkSite {
            site_id: format!("site-{id}"),
            study_id: format!("study-{id}"),
            capability_id: current_benchmark.capability_id.clone(),
            benchmark_world: current_benchmark.benchmark_world.clone(),
            metric_name: current_benchmark.metric_name.clone(),
            model_system: current_benchmark.model_system,
            artifact: artifact(&format!("artifact-{id}")),
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: 45,
            replicate_count: 4,
        };
        let candidate = |id: &str, score: u64, cost: u32| FederatedBenchmarkCandidate {
            candidate_id: format!("candidate-{id}"),
            site_id: format!("candidate-site-{id}"),
            study_id: format!("candidate-study-{id}"),
            independence_group: format!("group-{id}"),
            artifact: artifact(&format!("candidate-artifact-{id}")),
            baseline_score_milli: 500,
            expected_candidate_score_milli: score,
            uncertainty_milli: 30,
            replicate_count: 4,
            cost_units: cost,
            privacy_risk_milli: 100,
        };
        FederatedBenchmarkAdaptiveCampaignRequest {
            planning: FederatedBenchmarkSitePlannerRequest {
                benchmark,
                current_sites: vec![current("a", 600), current("b", 615)],
                candidates: vec![candidate("c", 625, 4), candidate("d", 900, 9)],
                budget_units: 12,
                max_new_sites: 2,
                beam_width: 8,
                privacy_budget_milli: 500,
                conservatism_milli: 500,
            },
            max_rounds: 2,
            max_retries: 1,
            stop_on_qualified: false,
            stop_on_negative: false,
            require_aggregate_only: true,
        }
    }

    #[test]
    fn planner_projection_becomes_observed_aggregate_campaign_and_replays() {
        let first = execute_federated_benchmark_adaptive_campaign_dry_run(&request()).unwrap();
        let second = execute_federated_benchmark_adaptive_campaign_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert!(!first.planned_site_order.is_empty());
        assert!(first.campaign.is_some());
        assert_eq!(
            first.projected_disposition,
            FederatedBenchmarkDisposition::Qualified
        );
        assert!(first
            .uncertainty
            .iter()
            .any(|item| item.contains("scenario") || item.contains("synthetic")));
    }

    #[test]
    fn non_aggregate_site_is_refused_before_planning() {
        let mut request = request();
        request.planning.current_sites[0].artifact.local_only = false;
        assert!(matches!(
            execute_federated_benchmark_adaptive_campaign_dry_run(&request),
            Err(FederatedBenchmarkAdaptiveCampaignError::Boundary(_))
        ));
    }
}
