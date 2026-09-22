//! Aggregate-only federated benchmark operating cycle for preclinical glioma research.
//!
//! This feature turns a consortium benchmark into an executable control loop: validate the
//! aggregate-only boundary, compute the current cross-site consensus, run the bounded follow-up
//! campaign, and return the next governance action. Institutions retain raw traces and execution
//! authority; only typed site aggregates enter this cycle.

use super::campaign::{
    execute_federated_benchmark_campaign, DryRunFederatedBenchmarkCampaignExecutor,
    FederatedBenchmarkCampaign, FederatedBenchmarkCampaignError,
    FederatedBenchmarkCampaignExecutor, FederatedBenchmarkCampaignRequest,
};
use super::consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkDisposition,
    FederatedBenchmarkError,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkOperatingCycleRequest {
    pub campaign: FederatedBenchmarkCampaignRequest,
    pub execution_mode: FederatedBenchmarkExecutionMode,
    pub require_aggregate_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkOperatingCycleDisposition {
    Qualified,
    Negative,
    Heterogeneous,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub preflight: FederatedBenchmarkConsensus,
    pub campaign: Option<FederatedBenchmarkCampaign>,
    pub simulation_only: bool,
    pub execution_mode: FederatedBenchmarkExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkOperatingCycleError {
    #[error("federated operating-cycle consensus failed: {0}")]
    Consensus(#[from] FederatedBenchmarkError),
    #[error("federated operating-cycle campaign failed: {0}")]
    Campaign(#[from] FederatedBenchmarkCampaignError),
    #[error("federated operating-cycle boundary blocked: {0}")]
    Boundary(String),
    #[error("federated operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedBenchmarkOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "preflight": output.preflight,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn disposition(
    preflight: FederatedBenchmarkDisposition,
    campaign: Option<&FederatedBenchmarkCampaign>,
) -> FederatedBenchmarkOperatingCycleDisposition {
    campaign.map_or_else(
        || match preflight {
            FederatedBenchmarkDisposition::Qualified => {
                FederatedBenchmarkOperatingCycleDisposition::Qualified
            }
            FederatedBenchmarkDisposition::Negative => {
                FederatedBenchmarkOperatingCycleDisposition::Negative
            }
            FederatedBenchmarkDisposition::Heterogeneous => {
                FederatedBenchmarkOperatingCycleDisposition::Heterogeneous
            }
            FederatedBenchmarkDisposition::Unresolved => {
                FederatedBenchmarkOperatingCycleDisposition::Blocked
            }
        },
        |campaign| match campaign.disposition {
            super::campaign::FederatedBenchmarkCampaignDisposition::Qualified => {
                FederatedBenchmarkOperatingCycleDisposition::Qualified
            }
            super::campaign::FederatedBenchmarkCampaignDisposition::Negative => {
                FederatedBenchmarkOperatingCycleDisposition::Negative
            }
            super::campaign::FederatedBenchmarkCampaignDisposition::Heterogeneous => {
                FederatedBenchmarkOperatingCycleDisposition::Heterogeneous
            }
            super::campaign::FederatedBenchmarkCampaignDisposition::Partial => {
                FederatedBenchmarkOperatingCycleDisposition::Partial
            }
            super::campaign::FederatedBenchmarkCampaignDisposition::BudgetBlocked
            | super::campaign::FederatedBenchmarkCampaignDisposition::Failed => {
                FederatedBenchmarkOperatingCycleDisposition::Blocked
            }
            super::campaign::FederatedBenchmarkCampaignDisposition::Unresolved => {
                FederatedBenchmarkOperatingCycleDisposition::Unresolved
            }
        },
    )
}

impl FederatedBenchmarkOperatingCycle {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "aggregate_boundary".to_string(),
                    "consensus_preflight".to_string(),
                    "federated_campaign".to_string(),
                    "governance_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.preflight.objective != self.objective
            || self.simulation_only
                != matches!(
                    self.execution_mode,
                    FederatedBenchmarkExecutionMode::LocalSimulation
                )
        {
            return Err(FederatedBenchmarkOperatingCycleError::InvalidOutput(
                "identity, phase order, objective binding, evidence ordering, or execution mode is invalid".into(),
            ));
        }
        self.preflight.validate().map_err(|error| {
            FederatedBenchmarkOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        if let Some(campaign) = &self.campaign {
            campaign.validate().map_err(|error| {
                FederatedBenchmarkOperatingCycleError::InvalidOutput(error.to_string())
            })?;
            if campaign.objective != self.objective
                || campaign.final_consensus.objective != self.objective
            {
                return Err(FederatedBenchmarkOperatingCycleError::InvalidOutput(
                    "campaign is not bound to the preflight benchmark objective".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkOperatingCycleError::InvalidOutput(
                "federated operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Validate the aggregate-only boundary, compute consensus, and execute the bounded campaign.
pub fn execute_federated_benchmark_operating_cycle<E: FederatedBenchmarkCampaignExecutor>(
    request: &FederatedBenchmarkOperatingCycleRequest,
    executor: &mut E,
) -> Result<FederatedBenchmarkOperatingCycle, FederatedBenchmarkOperatingCycleError> {
    if request.campaign.initial_sites.is_empty()
        || request.campaign.initial_sites.len() > 256
        || request.campaign.benchmark.minimum_sites == 0
    {
        return Err(FederatedBenchmarkOperatingCycleError::Boundary(
            "at least one bounded aggregate site and a positive minimum_sites gate are required"
                .into(),
        ));
    }
    if request.require_aggregate_only
        && request.campaign.initial_sites.iter().any(|site| {
            !site.artifact.local_only
                || site.artifact.contains_human_data
                || site.artifact.contains_direct_identifiers
        })
    {
        return Err(FederatedBenchmarkOperatingCycleError::Boundary(
            "federation requires local-only, non-human, non-identifying aggregate artifacts".into(),
        ));
    }
    let preflight =
        analyze_federated_benchmark(&request.campaign.benchmark, &request.campaign.initial_sites)?;
    let campaign = execute_federated_benchmark_campaign(&request.campaign, executor)?;
    let cycle_disposition = disposition(preflight.disposition, Some(&campaign));
    let mut negative_evidence = preflight.negative_evidence.clone();
    negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = preflight.uncertainty.clone();
    uncertainty.extend(campaign.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match cycle_disposition {
        FederatedBenchmarkOperatingCycleDisposition::Qualified => {
            "publish the aggregate benchmark with site-local provenance and open the next independent validation round".into()
        }
        FederatedBenchmarkOperatingCycleDisposition::Negative => {
            "preserve the negative benchmark and inspect model, metric, and site-specific failure modes before another round".into()
        }
        FederatedBenchmarkOperatingCycleDisposition::Heterogeneous => {
            "resolve cross-site heterogeneity and retain site-stratified results instead of reporting a pooled pass".into()
        }
        FederatedBenchmarkOperatingCycleDisposition::Partial => {
            "continue only the highest-information aggregate actions and keep incomplete coverage explicit".into()
        }
        FederatedBenchmarkOperatingCycleDisposition::Blocked => {
            "resolve aggregate-boundary, budget, or executor failures before federation continues".into()
        }
        FederatedBenchmarkOperatingCycleDisposition::Unresolved => {
            "obtain independent aggregate evidence before making a transportability claim".into()
        }
    };
    let mut output = FederatedBenchmarkOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.campaign.benchmark.objective.clone(),
        phase_order: vec![
            "aggregate_boundary".into(),
            "consensus_preflight".into(),
            "federated_campaign".into(),
            "governance_handoff".into(),
        ],
        preflight,
        campaign: Some(campaign),
        simulation_only: matches!(
            request.execution_mode,
            FederatedBenchmarkExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition: cycle_disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_federated_benchmark_operating_cycle_dry_run(
    request: &FederatedBenchmarkOperatingCycleRequest,
) -> Result<FederatedBenchmarkOperatingCycle, FederatedBenchmarkOperatingCycleError> {
    let mut executor = DryRunFederatedBenchmarkCampaignExecutor;
    execute_federated_benchmark_operating_cycle(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p12_federated_benchmarking::{
        FederatedBenchmarkAction, FederatedBenchmarkActionKind, FederatedBenchmarkRequest,
        FederatedBenchmarkSite,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn request() -> FederatedBenchmarkOperatingCycleRequest {
        let hash = ContentHash::of_bytes(b"federated-operating-cycle");
        let benchmark = FederatedBenchmarkRequest {
            objective: "compare invasion model improvements".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 2,
            effect_threshold_milli: 25,
            max_i2_milli: 500,
            min_signal_to_noise_milli: 100,
            max_site_spread_milli: 500,
            max_leave_one_out_shift_milli: 500,
        };
        let site = FederatedBenchmarkSite {
            site_id: "seed-site".into(),
            study_id: "seed-study".into(),
            capability_id: benchmark.capability_id.clone(),
            benchmark_world: benchmark.benchmark_world.clone(),
            metric_name: benchmark.metric_name.clone(),
            model_system: benchmark.model_system,
            artifact: LocalArtifactRef {
                artifact_id: "seed-artifact".into(),
                content_hash: hash,
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: 620,
            uncertainty_milli: 40,
            replicate_count: 3,
        };
        FederatedBenchmarkOperatingCycleRequest {
            campaign: FederatedBenchmarkCampaignRequest {
                benchmark,
                initial_sites: vec![site],
                actions: vec![FederatedBenchmarkAction {
                    action_id: "expand-site".into(),
                    kind: FederatedBenchmarkActionKind::ExpandCoverage,
                    target_site_id: None,
                    cost_units: 1,
                    expected_information_milli: 900,
                    expected_effect_milli: 100,
                    feasibility_milli: 900,
                    risk_milli: 50,
                    requested_replicates: 3,
                }],
                budget_units: 1,
                max_rounds: 2,
                max_retries: 1,
                stop_on_qualified: false,
                stop_on_negative: false,
            },
            execution_mode: FederatedBenchmarkExecutionMode::LocalSimulation,
            require_aggregate_only: true,
        }
    }

    #[test]
    fn operating_cycle_preserves_aggregate_boundary_and_replay() {
        let first = execute_federated_benchmark_operating_cycle_dry_run(&request()).unwrap();
        let second = execute_federated_benchmark_operating_cycle_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.campaign.as_ref().unwrap().sites.len(), 2);
        assert_eq!(first.phase_order[0], "aggregate_boundary");
        first.validate().unwrap();
    }
}
