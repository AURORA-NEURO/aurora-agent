//! End-to-end mechanism-exploration orchestration for preclinical glioma research.
//!
//! P05's operating cycle turns competing mechanistic models into a bounded local campaign and
//! then compiles the resulting information gain into the next typed assay work package.  The
//! campaign executor remains caller-owned: MCP uses the deterministic sandbox adapter while an
//! institution can provide an instrument, imaging, omics, or computation adapter with its own
//! policy and authorization gates.  A planned assay is never treated as an observation.

use super::action_planner::{
    compile_mechanism_action_plan, MechanismActionPlan, MechanismActionPlannerConfig,
    MechanismActionPlannerError,
};
use super::discrimination_campaign::{
    execute_glioma_mechanism_discrimination_campaign, MechanismDiscriminationCampaign,
    MechanismDiscriminationCampaignError, MechanismDiscriminationCampaignExecutor,
    MechanismDiscriminationCampaignRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismOperatingCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismOperatingCycleRequest {
    pub campaign: MechanismDiscriminationCampaignRequest,
    pub action_plan: MechanismActionPlannerConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismOperatingCycleDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    NoActions,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub campaign: MechanismDiscriminationCampaign,
    pub action_plan: MechanismActionPlan,
    pub selected_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: MechanismOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismOperatingCycleError {
    #[error("mechanism operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism operating-cycle campaign failed: {0}")]
    Campaign(#[from] MechanismDiscriminationCampaignError),
    #[error("mechanism operating-cycle action planning failed: {0}")]
    ActionPlan(#[from] MechanismActionPlannerError),
    #[error("mechanism operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "campaign": output.campaign,
        "action_plan": output.action_plan,
        "selected_action_order": output.selected_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
    })
}

impl MechanismOperatingCycle {
    pub fn validate(&self) -> Result<(), MechanismOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "mechanism_discrimination_campaign".to_string(),
                    "action_plan_compilation".to_string(),
                ]
            || !canonical(&self.selected_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.campaign.objective != self.objective
            || self.action_plan.source_discrimination_digest
                != self.campaign.final_discrimination.digest
            || !self.simulation_only
        {
            return Err(MechanismOperatingCycleError::InvalidOutput(
                "identity, phase order, objective, digest binding, simulation, or ordering is invalid".into(),
            ));
        }
        self.campaign
            .validate()
            .map_err(|error| MechanismOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.action_plan
            .validate()
            .map_err(|error| MechanismOperatingCycleError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismOperatingCycleError::InvalidOutput(
                "mechanism operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn disposition(
    campaign: super::discrimination_campaign::MechanismDiscriminationCampaignDisposition,
) -> MechanismOperatingCycleDisposition {
    match campaign {
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::Qualified => {
            MechanismOperatingCycleDisposition::Qualified
        }
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::Partial => {
            MechanismOperatingCycleDisposition::Partial
        }
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::BudgetBlocked => {
            MechanismOperatingCycleDisposition::BudgetBlocked
        }
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::Failed => {
            MechanismOperatingCycleDisposition::Failed
        }
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::NoActions => {
            MechanismOperatingCycleDisposition::NoActions
        }
        super::discrimination_campaign::MechanismDiscriminationCampaignDisposition::Unresolved => {
            MechanismOperatingCycleDisposition::Unresolved
        }
    }
}

/// Run a bounded mechanism-discrimination campaign and compile its final information gain into
/// the next local, typed assay work package.
pub fn execute_glioma_mechanism_operating_cycle<E: MechanismDiscriminationCampaignExecutor>(
    request: &MechanismOperatingCycleRequest,
    executor: &mut E,
) -> Result<MechanismOperatingCycle, MechanismOperatingCycleError> {
    if request.campaign.discrimination.objective.trim().is_empty() {
        return Err(MechanismOperatingCycleError::InvalidRequest(
            "objective must not be empty".into(),
        ));
    }
    if request.campaign.discrimination.model_system != request.action_plan.model_system {
        return Err(MechanismOperatingCycleError::InvalidRequest(
            "campaign and action-plan model systems must match".into(),
        ));
    }
    let campaign = execute_glioma_mechanism_discrimination_campaign(&request.campaign, executor)?;
    let action_plan =
        compile_mechanism_action_plan(&campaign.final_discrimination, &request.action_plan)?;
    let mut negative_evidence = campaign.negative_evidence.clone();
    negative_evidence.extend(action_plan.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = campaign.uncertainty.clone();
    uncertainty.extend(action_plan.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = MechanismOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: campaign.objective.clone(),
        phase_order: vec![
            "mechanism_discrimination_campaign".into(),
            "action_plan_compilation".into(),
        ],
        selected_action_order: action_plan.action_order.clone(),
        simulation_only: true,
        disposition: disposition(campaign.disposition),
        campaign,
        action_plan,
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-mechanism-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::discrimination::{
        MechanismDiscriminatorAction, MechanismFeatureObservation, MechanismHypothesis,
        MechanismPrediction,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use std::collections::BTreeMap;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn request() -> MechanismOperatingCycleRequest {
        MechanismOperatingCycleRequest {
            campaign: MechanismDiscriminationCampaignRequest {
                discrimination: super::super::discrimination::MechanismDiscriminationRequest {
                    objective: "resolve glioma invasion mechanisms".into(),
                    model_system: GliomaModelSystem::Organoid,
                    min_shared_features: 2,
                    max_mechanisms: 4,
                    max_actions: 2,
                    min_information_gain_milli: 10,
                },
                hypotheses: vec![
                    MechanismHypothesis {
                        mechanism_id: "motility".into(),
                        statement: "motility drives invasion".into(),
                        predictions: vec![
                            MechanismPrediction {
                                feature_id: "f1".into(),
                                predicted_milli: 100,
                                uncertainty_milli: 10,
                            },
                            MechanismPrediction {
                                feature_id: "f2".into(),
                                predicted_milli: 200,
                                uncertainty_milli: 10,
                            },
                        ],
                    },
                    MechanismHypothesis {
                        mechanism_id: "matrix".into(),
                        statement: "matrix remodeling drives invasion".into(),
                        predictions: vec![
                            MechanismPrediction {
                                feature_id: "f1".into(),
                                predicted_milli: 400,
                                uncertainty_milli: 10,
                            },
                            MechanismPrediction {
                                feature_id: "f2".into(),
                                predicted_milli: 500,
                                uncertainty_milli: 10,
                            },
                        ],
                    },
                ],
                actions: vec![MechanismDiscriminatorAction {
                    action_id: "measure-f1".into(),
                    feature_id: "f1".into(),
                    predicted_milli_by_mechanism: BTreeMap::from([
                        ("matrix".into(), 500),
                        ("motility".into(), 100),
                    ]),
                    measurement_uncertainty_milli: 20,
                    feasibility_milli: 1_000,
                    cost_units: 1,
                }],
                observations: vec![
                    MechanismFeatureObservation {
                        feature_id: "f1".into(),
                        observed_milli: 100,
                        uncertainty_milli: 10,
                        artifact: LocalArtifactRef {
                            artifact_id: "seed".into(),
                            content_hash: hash("seed"),
                            content_type: "application/json".into(),
                            local_only: true,
                            contains_human_data: false,
                            contains_direct_identifiers: false,
                        },
                    },
                    MechanismFeatureObservation {
                        feature_id: "f2".into(),
                        observed_milli: 200,
                        uncertainty_milli: 10,
                        artifact: LocalArtifactRef {
                            artifact_id: "seed-f2".into(),
                            content_hash: hash("seed-f2"),
                            content_type: "application/json".into(),
                            local_only: true,
                            contains_human_data: false,
                            contains_direct_identifiers: false,
                        },
                    },
                ],
                budget_units: 1,
                max_rounds: 3,
                max_retries: 1,
                stop_on_qualified: true,
            },
            action_plan: MechanismActionPlannerConfig {
                model_system: GliomaModelSystem::Organoid,
                modality: GliomaModality::Transcriptomics,
                max_actions: 2,
            },
        }
    }

    #[test]
    fn operating_cycle_replays_and_binds_next_work_package() {
        let request = request();
        let mut first_executor =
            super::super::discrimination_campaign::DryRunMechanismDiscriminationCampaignExecutor;
        let mut second_executor =
            super::super::discrimination_campaign::DryRunMechanismDiscriminationCampaignExecutor;
        let first =
            execute_glioma_mechanism_operating_cycle(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_mechanism_operating_cycle(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.phase_order.len(), 2);
        assert_eq!(
            first.action_plan.source_discrimination_digest,
            first.campaign.final_discrimination.digest
        );
        first.validate().unwrap();
    }

    #[test]
    fn operating_cycle_refuses_model_mismatch_before_execution() {
        let mut request = request();
        request.action_plan.model_system = GliomaModelSystem::CellLine;
        let mut executor =
            super::super::discrimination_campaign::DryRunMechanismDiscriminationCampaignExecutor;
        let error = execute_glioma_mechanism_operating_cycle(&request, &mut executor).unwrap_err();
        assert!(error.to_string().contains("model systems must match"));
    }
}
