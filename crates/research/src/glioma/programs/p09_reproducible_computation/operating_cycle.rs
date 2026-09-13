//! Intent-to-computation operating cycle for autonomous preclinical glioma research.
//!
//! P09 already provides the individual workflow compiler, portfolio planner, placement planner,
//! executor, and multi-round campaign. This feature composes the researcher-facing path into one
//! bounded capability: compile a typed multimodal intent, enforce its declared resource gate,
//! execute the dependency-closed computation campaign through a caller-owned worker, and return a
//! replayable operator handoff. It never reads raw payloads, downloads code, or promotes a local
//! computation artifact into biological evidence by itself.

use super::campaign::{
    execute_glioma_computation_campaign, GliomaComputationCampaign,
    GliomaComputationCampaignDisposition, GliomaComputationCampaignError,
    StaticGliomaComputationPlanner,
};
use super::execution::{DryRunGliomaComputationExecutor, GliomaComputationExecutor};
use super::workflow::{
    compile_glioma_computation_workflow, GliomaComputationWorkflow, GliomaComputationWorkflowError,
    GliomaComputationWorkflowRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationOperatingCycleRequest {
    pub workflow: GliomaComputationWorkflowRequest,
    pub require_within_resources: bool,
    pub execution_mode: ComputationExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaComputationOperatingCycleDisposition {
    Executed,
    Negative,
    Partial,
    Blocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub phase_order: Vec<String>,
    pub workflow: GliomaComputationWorkflow,
    pub campaign: Option<GliomaComputationCampaign>,
    pub simulation_only: bool,
    pub execution_mode: ComputationExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaComputationOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaComputationOperatingCycleError {
    #[error("computation operating-cycle workflow failed: {0}")]
    Workflow(#[from] GliomaComputationWorkflowError),
    #[error("computation operating-cycle campaign failed: {0}")]
    Campaign(#[from] GliomaComputationCampaignError),
    #[error("computation operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaComputationOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "phase_order": output.phase_order,
        "workflow": output.workflow,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn campaign_disposition(
    campaign: &GliomaComputationCampaign,
) -> GliomaComputationOperatingCycleDisposition {
    match campaign.disposition {
        GliomaComputationCampaignDisposition::Completed
            if !campaign.negative_evidence.is_empty() =>
        {
            GliomaComputationOperatingCycleDisposition::Negative
        }
        GliomaComputationCampaignDisposition::Completed => {
            GliomaComputationOperatingCycleDisposition::Executed
        }
        GliomaComputationCampaignDisposition::Partial => {
            GliomaComputationOperatingCycleDisposition::Partial
        }
        GliomaComputationCampaignDisposition::Failed => {
            GliomaComputationOperatingCycleDisposition::Failed
        }
        GliomaComputationCampaignDisposition::Blocked => {
            GliomaComputationOperatingCycleDisposition::Blocked
        }
    }
}

impl GliomaComputationOperatingCycle {
    pub fn validate(&self) -> Result<(), GliomaComputationOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.phase_order
                != [
                    "workflow_compile".to_string(),
                    "resource_gate".to_string(),
                    "computation_campaign".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.simulation_only
                != matches!(
                    self.execution_mode,
                    ComputationExecutionMode::LocalSimulation
                )
            || self.workflow.objective != self.objective
            || self.workflow.study_id != self.study_id
        {
            return Err(GliomaComputationOperatingCycleError::InvalidOutput(
                "identity, phase order, workflow binding, execution mode, or canonical evidence is invalid".into(),
            ));
        }
        self.workflow.validate().map_err(|error| {
            GliomaComputationOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        if let Some(campaign) = &self.campaign {
            campaign.validate().map_err(|error| {
                GliomaComputationOperatingCycleError::InvalidOutput(error.to_string())
            })?;
            if campaign.objective != self.objective
                || campaign.model_system != self.workflow.model_system
                || campaign.replay_identity != self.workflow.replay_identity
            {
                return Err(GliomaComputationOperatingCycleError::InvalidOutput(
                    "campaign is not bound to the compiled workflow identity".into(),
                ));
            }
        } else if !matches!(
            self.disposition,
            GliomaComputationOperatingCycleDisposition::Blocked
        ) {
            return Err(GliomaComputationOperatingCycleError::InvalidOutput(
                "only a resource-blocked cycle may omit campaign execution".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaComputationOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaComputationOperatingCycleError::InvalidOutput(
                "computation operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a multimodal computation intent and execute it through a caller-owned planner/worker.
pub fn execute_glioma_computation_operating_cycle<
    P: super::campaign::GliomaComputationPlanner,
    E: GliomaComputationExecutor,
>(
    request: &GliomaComputationOperatingCycleRequest,
    planner: &mut P,
    executor: &mut E,
) -> Result<GliomaComputationOperatingCycle, GliomaComputationOperatingCycleError> {
    let workflow = compile_glioma_computation_workflow(&request.workflow)?;
    let mut uncertainty = workflow.uncertainty.clone();
    let resource_blocked = request.require_within_resources && !workflow.within_declared_resources;
    let (campaign, disposition, next_operator_action, negative_evidence) = if resource_blocked {
        uncertainty.push("workflow-resource-gate-blocked-execution".into());
        (
            None,
            GliomaComputationOperatingCycleDisposition::Blocked,
            "increase the declared compute or duration budget, then recompile the workflow".into(),
            vec!["workflow-resource-gate-not-cleared".into()],
        )
    } else {
        let campaign_request = workflow.campaign_request(&request.workflow)?;
        let campaign = execute_glioma_computation_campaign(&campaign_request, planner, executor)?;
        let disposition = campaign_disposition(&campaign);
        let next = match disposition {
            GliomaComputationOperatingCycleDisposition::Executed
            | GliomaComputationOperatingCycleDisposition::Negative => {
                "inspect replayed local artifacts and route qualified outputs to interpretation"
                    .into()
            }
            GliomaComputationOperatingCycleDisposition::Partial => {
                "review partial tasks and schedule the highest-value reproducible continuation"
                    .into()
            }
            GliomaComputationOperatingCycleDisposition::Blocked
            | GliomaComputationOperatingCycleDisposition::Unresolved => {
                "resolve missing dependencies or evidence before rerunning computation".into()
            }
            GliomaComputationOperatingCycleDisposition::Failed => {
                "inspect the failed worker task and preserve its replay and negative evidence"
                    .into()
            }
        };
        (
            Some(campaign.clone()),
            disposition,
            next,
            campaign.negative_evidence.clone(),
        )
    };
    if let Some(campaign) = &campaign {
        uncertainty.extend(campaign.uncertainty.iter().cloned());
    }
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_evidence = negative_evidence;
    if let Some(campaign) = &campaign {
        negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut output = GliomaComputationOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: workflow.objective.clone(),
        study_id: workflow.study_id.clone(),
        phase_order: vec![
            "workflow_compile".into(),
            "resource_gate".into(),
            "computation_campaign".into(),
            "operator_handoff".into(),
        ],
        workflow,
        campaign,
        simulation_only: matches!(
            request.execution_mode,
            ComputationExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaComputationOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Run the operating cycle in a deterministic sandbox with the built-in planner and worker.
pub fn execute_glioma_computation_operating_cycle_dry_run(
    request: &GliomaComputationOperatingCycleRequest,
) -> Result<GliomaComputationOperatingCycle, GliomaComputationOperatingCycleError> {
    let mut planner = StaticGliomaComputationPlanner;
    let mut executor = DryRunGliomaComputationExecutor;
    execute_glioma_computation_operating_cycle(request, &mut planner, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution::ComputationOperation;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn request() -> GliomaComputationOperatingCycleRequest {
        GliomaComputationOperatingCycleRequest {
            workflow: GliomaComputationWorkflowRequest {
                objective: "profile invasive organoid state across modalities".into(),
                study_id: "study-glioma-01".into(),
                model_system: GliomaModelSystem::Organoid,
                modalities: vec![GliomaModality::Transcriptomics, GliomaModality::Imaging],
                operations: vec![ComputationOperation::Export, ComputationOperation::ModelFit],
                input_artifact_ids: vec!["artifact-imaging".into(), "artifact-rna".into()],
                budget_units: 100,
                duration_ticks: 500,
                max_tasks: 64,
                max_modalities: 4,
                min_modalities: 2,
                information_weight_milli: 5,
                uncertainty_weight_milli: 4,
                coverage_weight_milli: 3,
                cost_penalty_milli: 1,
                duration_penalty_milli: 1,
                require_deterministic: true,
                max_rounds: 4,
                max_retries: 1,
                allow_cache: true,
                require_local_artifacts: true,
                cache: Vec::new(),
                replay_identity: ContentHash::of_bytes(b"workflow-replay"),
            },
            require_within_resources: true,
            execution_mode: ComputationExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn operating_cycle_compiles_executes_and_replays() {
        let request = request();
        let first = execute_glioma_computation_operating_cycle_dry_run(&request).unwrap();
        let second = execute_glioma_computation_operating_cycle_dry_run(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            GliomaComputationOperatingCycleDisposition::Executed
        );
        assert!(first.workflow.candidates.len() > 1);
        assert!(first.campaign.as_ref().unwrap().completed_order.len() > 1);
        first.validate().unwrap();
    }

    #[test]
    fn resource_gate_blocks_without_worker_dispatch() {
        let mut request = request();
        request.workflow.budget_units = 1;
        let output = execute_glioma_computation_operating_cycle_dry_run(&request).unwrap();
        assert_eq!(
            output.disposition,
            GliomaComputationOperatingCycleDisposition::Blocked
        );
        assert!(output.campaign.is_none());
        assert!(output
            .uncertainty
            .contains(&"workflow-resource-gate-blocked-execution".to_string()));
        output.validate().unwrap();
    }
}
