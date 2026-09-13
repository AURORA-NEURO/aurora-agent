//! End-to-end experiment-design orchestration for preclinical glioma research.
//!
//! This feature is the P06 bridge from a typed experiment objective to a bounded, replanning
//! campaign. It plans a mechanism-aware assay batch, executes only through a caller-owned local
//! adapter, incorporates the returned observations, and emits the final design state. The MCP
//! adapter is metadata-only; an institution can replace it with a governed assay, imaging, or
//! computation worker without changing the deterministic planning contract.

use super::campaign::{
    execute_glioma_closed_loop_campaign, plan_glioma_closed_loop_campaign, CampaignAction,
    CampaignExecutionFailure, CampaignMechanism, CampaignObservation, ClosedLoopCampaign,
    ClosedLoopCampaignError, ClosedLoopCampaignExecution, ClosedLoopCampaignRequest,
    GliomaCampaignExecutor,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaExperimentOperatingCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentOperatingCycleRequest {
    pub planning: ClosedLoopCampaignRequest,
    pub mechanisms: Vec<CampaignMechanism>,
    pub actions: Vec<CampaignAction>,
    pub observations: Vec<CampaignObservation>,
    pub simulation_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentOperatingCycleDisposition {
    Qualified,
    Partial,
    Converged,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub initial_plan: ClosedLoopCampaign,
    pub execution: ClosedLoopCampaignExecution,
    pub final_plan: ClosedLoopCampaign,
    pub simulation_only: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ExperimentOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentOperatingCycleError {
    #[error("experiment operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("experiment operating-cycle planning or execution failed: {0}")]
    Campaign(#[from] ClosedLoopCampaignError),
    #[error("experiment operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("experiment operating-cycle digest failed: {0}")]
    Digest(String),
}

/// Deterministic metadata-only P06 adapter used by MCP and local rehearsal tests.
#[derive(Debug, Default)]
pub struct DryRunExperimentOperatingCycleExecutor;

impl GliomaCampaignExecutor for DryRunExperimentOperatingCycleExecutor {
    fn execute_action(
        &mut self,
        action: &CampaignAction,
        round: u16,
    ) -> Result<Vec<CampaignObservation>, CampaignExecutionFailure> {
        let observed_milli = action
            .predicted_milli_by_mechanism
            .values()
            .copied()
            .max()
            .unwrap_or_default();
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "round": round,
            "observed_milli": observed_milli,
            "simulation_only": true,
        }))
        .map_err(|error| CampaignExecutionFailure {
            reason: format!("dry-run experiment observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(vec![CampaignObservation {
            action_id: action.action_id.clone(),
            observed_milli,
            uncertainty_milli: action.measurement_uncertainty_milli,
            replicate_index: u32::from(round),
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-experiment:{}:{round}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.experiment-observation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }])
    }
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ExperimentOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "initial_plan": output.initial_plan,
        "execution": output.execution,
        "final_plan": output.final_plan,
        "simulation_only": output.simulation_only,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl ExperimentOperatingCycle {
    pub fn validate(&self) -> Result<(), ExperimentOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "experiment_planning".to_string(),
                    "local_execution".to_string(),
                    "final_replan".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.initial_plan.objective != self.objective
            || self.final_plan.objective != self.objective
        {
            return Err(ExperimentOperatingCycleError::InvalidOutput(
                "identity, phase order, objective, ordering, or execution binding is invalid"
                    .into(),
            ));
        }
        self.initial_plan
            .validate()
            .map_err(|error| ExperimentOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.execution
            .validate()
            .map_err(|error| ExperimentOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.final_plan
            .validate()
            .map_err(|error| ExperimentOperatingCycleError::InvalidOutput(error.to_string()))?;
        if self.final_plan.digest != self.execution.final_campaign.digest {
            return Err(ExperimentOperatingCycleError::InvalidOutput(
                "final plan is not bound to execution final campaign".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ExperimentOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ExperimentOperatingCycleError::InvalidOutput(
                "experiment operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn disposition(
    plan: super::campaign::ClosedLoopCampaignDisposition,
) -> ExperimentOperatingCycleDisposition {
    match plan {
        super::campaign::ClosedLoopCampaignDisposition::Qualified => {
            ExperimentOperatingCycleDisposition::Qualified
        }
        super::campaign::ClosedLoopCampaignDisposition::Partial => {
            ExperimentOperatingCycleDisposition::Partial
        }
        super::campaign::ClosedLoopCampaignDisposition::Converged => {
            ExperimentOperatingCycleDisposition::Converged
        }
        super::campaign::ClosedLoopCampaignDisposition::BudgetBlocked => {
            ExperimentOperatingCycleDisposition::BudgetBlocked
        }
        super::campaign::ClosedLoopCampaignDisposition::Unresolved => {
            ExperimentOperatingCycleDisposition::Unresolved
        }
    }
}

/// Plan, execute, and replan a bounded mechanism-aware experiment campaign.
pub fn execute_glioma_experiment_operating_cycle<E: GliomaCampaignExecutor>(
    request: &ExperimentOperatingCycleRequest,
    executor: &mut E,
) -> Result<ExperimentOperatingCycle, ExperimentOperatingCycleError> {
    if request.planning.objective.trim().is_empty()
        || request.mechanisms.is_empty()
        || request.actions.is_empty()
    {
        return Err(ExperimentOperatingCycleError::InvalidRequest(
            "objective, mechanism declarations, and action declarations are required".into(),
        ));
    }
    let initial_plan = plan_glioma_closed_loop_campaign(
        &request.planning,
        &request.mechanisms,
        &request.actions,
        &request.observations,
    )?;
    let execution = execute_glioma_closed_loop_campaign(
        &request.planning,
        &request.mechanisms,
        &request.actions,
        &request.observations,
        executor,
    )?;
    let final_plan = execution.final_campaign.clone();
    let mut negative_evidence = initial_plan.negative_evidence.clone();
    negative_evidence.extend(final_plan.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = initial_plan.uncertainty.clone();
    uncertainty.extend(final_plan.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let final_disposition = disposition(final_plan.disposition);
    let mut output = ExperimentOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.planning.objective.clone(),
        phase_order: vec![
            "experiment_planning".into(),
            "local_execution".into(),
            "final_replan".into(),
        ],
        initial_plan,
        execution,
        final_plan,
        simulation_only: request.simulation_only,
        negative_evidence,
        uncertainty,
        disposition: final_disposition,
        digest: ContentHash::of_bytes(b"unsealed-experiment-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ExperimentOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn request() -> ExperimentOperatingCycleRequest {
        ExperimentOperatingCycleRequest {
            planning: ClosedLoopCampaignRequest {
                objective: "discriminate invasion mechanisms in glioma organoids".into(),
                model_system: crate::glioma_engine::GliomaModelSystem::Organoid,
                max_rounds: 3,
                max_actions_per_round: 1,
                budget_units: 8,
                min_information_gain_milli: 100,
                information_weight_milli: 700,
                effect_weight_milli: 100,
                feasibility_weight_milli: 200,
                risk_penalty_milli: 200,
                stop_concentration_milli: 900,
            },
            mechanisms: vec![
                CampaignMechanism {
                    mechanism_id: "integrin".into(),
                    prior_milli: 500,
                },
                CampaignMechanism {
                    mechanism_id: "hypoxia".into(),
                    prior_milli: 500,
                },
            ],
            actions: vec![
                CampaignAction {
                    action_id: "assay-invasion".into(),
                    feature_id: "invasion-score".into(),
                    label: "organoid invasion assay".into(),
                    predicted_milli_by_mechanism: BTreeMap::from([
                        ("integrin".into(), 800),
                        ("hypoxia".into(), 100),
                    ]),
                    measurement_uncertainty_milli: 50,
                    feasibility_milli: 900,
                    expected_effect_milli: 600,
                    cost_units: 4,
                    risk_milli: 100,
                    max_replicates: 1,
                },
                CampaignAction {
                    action_id: "assay-oxygen".into(),
                    feature_id: "oxygen-response".into(),
                    label: "oxygen response assay".into(),
                    predicted_milli_by_mechanism: BTreeMap::from([
                        ("integrin".into(), 400),
                        ("hypoxia".into(), 700),
                    ]),
                    measurement_uncertainty_milli: 50,
                    feasibility_milli: 800,
                    expected_effect_milli: 300,
                    cost_units: 4,
                    risk_milli: 100,
                    max_replicates: 1,
                },
            ],
            observations: Vec::new(),
            simulation_only: true,
        }
    }

    #[test]
    fn operating_cycle_replans_from_local_observations_and_replays() {
        let request = request();
        let mut first_executor = DryRunExperimentOperatingCycleExecutor;
        let mut second_executor = DryRunExperimentOperatingCycleExecutor;
        let first =
            execute_glioma_experiment_operating_cycle(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_experiment_operating_cycle(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.phase_order.len(), 3);
        assert_eq!(
            first.final_plan.digest,
            first.execution.final_campaign.digest
        );
        assert!(!first.execution.rounds.is_empty());
        first.validate().unwrap();
    }

    #[test]
    fn operating_cycle_rejects_empty_action_registry() {
        let mut request = request();
        request.actions.clear();
        let mut executor = DryRunExperimentOperatingCycleExecutor;
        let error = execute_glioma_experiment_operating_cycle(&request, &mut executor).unwrap_err();
        assert!(error.to_string().contains("action declarations"));
    }
}
