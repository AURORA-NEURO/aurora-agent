//! Execute a bounded autonomous evidence-gap cycle for preclinical glioma research.
//!
//! This controller is the first end-to-end handoff between P02 and P01: compile typed knowledge
//! debt, plan a source-diverse acquisition portfolio, and execute the selected local portfolio
//! through a caller-owned adapter. Each phase remains independently validated and content
//! addressed. The controller never opens a network connection, moves protected data, or promotes
//! a dry-run result into biological evidence.

use super::claim_frontier::KnowledgeFrontier;
use super::gap_compiler::{
    compile_glioma_knowledge_gaps, KnowledgeGapCompilerError, KnowledgeGapCompilerRequest,
    KnowledgeGapPortfolio, KnowledgeGapPortfolioDisposition,
};
use super::knowledge_graph::TypedKnowledge;
use crate::glioma::programs::p01_evidence_surveillance::{
    execute_glioma_evidence_acquisition_campaign, plan_glioma_evidence_acquisition,
    EvidenceAcquisitionCampaign, EvidenceAcquisitionCampaignDisposition,
    EvidenceAcquisitionCampaignError, EvidenceAcquisitionCampaignRequest,
    EvidenceAcquisitionExecutor, EvidenceAcquisitionPlan, EvidenceAcquisitionRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousGapCycle1@1";
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousGapCycleRequest {
    pub gap: KnowledgeGapCompilerRequest,
    pub planning: EvidenceAcquisitionRequest,
    pub execution_budget_units: u64,
    pub execution_max_retries: u8,
    pub stop_on_negative: bool,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomousGapCycleDisposition {
    Completed,
    Partial,
    Blocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousGapCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub gap_portfolio: KnowledgeGapPortfolio,
    pub acquisition_plan: EvidenceAcquisitionPlan,
    pub campaign: EvidenceAcquisitionCampaign,
    pub simulation_only: bool,
    pub disposition: AutonomousGapCycleDisposition,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutonomousGapCycleError {
    #[error("autonomous gap-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("autonomous gap-cycle compilation failed: {0}")]
    Compilation(#[from] KnowledgeGapCompilerError),
    #[error("autonomous gap-cycle planning failed: {0}")]
    Planning(String),
    #[error("autonomous gap-cycle execution failed: {0}")]
    Execution(#[from] EvidenceAcquisitionCampaignError),
    #[error("autonomous gap-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("autonomous gap-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AutonomousGapCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "gap_portfolio": output.gap_portfolio,
        "acquisition_plan": output.acquisition_plan,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
    })
}

impl AutonomousGapCycle {
    pub fn validate(&self) -> Result<(), AutonomousGapCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "gap_compilation".to_string(),
                    "acquisition_planning".to_string(),
                    "acquisition_campaign".to_string(),
                ]
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.gap_portfolio.objective != self.objective
            || self.acquisition_plan.objective != self.objective
            || self.campaign.objective != self.objective
            || self.simulation_only != self.campaign.simulation_only
            || self.digest.as_str().len() != 64
        {
            return Err(AutonomousGapCycleError::InvalidOutput(
                "identity, phase order, objective, nested outputs, or digest fields are invalid"
                    .into(),
            ));
        }
        self.gap_portfolio
            .validate()
            .map_err(|error| AutonomousGapCycleError::InvalidOutput(error.to_string()))?;
        self.acquisition_plan
            .validate()
            .map_err(|error| AutonomousGapCycleError::InvalidOutput(error.to_string()))?;
        self.campaign
            .validate()
            .map_err(|error| AutonomousGapCycleError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AutonomousGapCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AutonomousGapCycleError::InvalidOutput(
                "autonomous gap-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn disposition(
    portfolio: KnowledgeGapPortfolioDisposition,
    campaign: EvidenceAcquisitionCampaignDisposition,
) -> AutonomousGapCycleDisposition {
    if matches!(portfolio, KnowledgeGapPortfolioDisposition::Blocked)
        || matches!(campaign, EvidenceAcquisitionCampaignDisposition::Blocked)
    {
        AutonomousGapCycleDisposition::Blocked
    } else {
        match campaign {
            EvidenceAcquisitionCampaignDisposition::Completed => {
                AutonomousGapCycleDisposition::Completed
            }
            EvidenceAcquisitionCampaignDisposition::Failed => AutonomousGapCycleDisposition::Failed,
            EvidenceAcquisitionCampaignDisposition::Partial
            | EvidenceAcquisitionCampaignDisposition::Negative => {
                AutonomousGapCycleDisposition::Partial
            }
            EvidenceAcquisitionCampaignDisposition::Unresolved => {
                AutonomousGapCycleDisposition::Unresolved
            }
            EvidenceAcquisitionCampaignDisposition::Blocked => {
                AutonomousGapCycleDisposition::Blocked
            }
        }
    }
}

/// Compile P02 knowledge debt, plan P01 acquisition work, and execute the selected local
/// portfolio. A production caller supplies the institution-local executor; MCP uses a dry-run
/// executor and therefore returns simulation metadata rather than biological evidence.
pub fn execute_glioma_autonomous_gap_cycle<E: EvidenceAcquisitionExecutor>(
    request: &AutonomousGapCycleRequest,
    knowledge: &TypedKnowledge,
    frontier: &KnowledgeFrontier,
    executor: &mut E,
) -> Result<AutonomousGapCycle, AutonomousGapCycleError> {
    if request.gap.objective != request.planning.objective
        || request.execution_budget_units == 0
        || request.execution_max_retries > MAX_RETRIES
    {
        return Err(AutonomousGapCycleError::InvalidRequest(
            "gap/planning objectives must match, execution budget must be positive, and retries must be bounded".into(),
        ));
    }
    let gap_portfolio = compile_glioma_knowledge_gaps(&request.gap, knowledge, frontier)?;
    let acquisition_plan =
        plan_glioma_evidence_acquisition(&request.planning, &gap_portfolio.candidates)
            .map_err(|error| AutonomousGapCycleError::Planning(error.to_string()))?;
    let campaign_request = EvidenceAcquisitionCampaignRequest {
        objective: request.planning.objective.clone(),
        plan: acquisition_plan.clone(),
        candidates: gap_portfolio.candidates.clone(),
        budget_units: request.execution_budget_units,
        max_retries: request.execution_max_retries,
        stop_on_negative: request.stop_on_negative,
        require_artifacts: request.require_artifacts,
    };
    let campaign = execute_glioma_evidence_acquisition_campaign(&campaign_request, executor)?;
    let mut uncertainty = gap_portfolio.uncertainty.clone();
    uncertainty.extend(acquisition_plan.uncertainty.iter().cloned());
    uncertainty.extend(campaign.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_evidence = gap_portfolio.negative_evidence.clone();
    negative_evidence.extend(acquisition_plan.negative_evidence.iter().cloned());
    negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut output = AutonomousGapCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.planning.objective.clone(),
        phase_order: vec![
            "gap_compilation".into(),
            "acquisition_planning".into(),
            "acquisition_campaign".into(),
        ],
        simulation_only: campaign.simulation_only,
        disposition: disposition(gap_portfolio.disposition, campaign.disposition),
        gap_portfolio,
        acquisition_plan,
        campaign,
        uncertainty,
        negative_evidence,
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-gap-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AutonomousGapCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p01_evidence_surveillance::{
        DryRunEvidenceAcquisitionExecutor, EvidenceAcquisitionSourceKind,
    };
    use crate::glioma::programs::p02_evidence_knowledge::claim_frontier::{
        prioritize_knowledge_frontier, KnowledgeFrontierRequest,
    };
    use crate::glioma::programs::p02_evidence_knowledge::gap_compiler::KnowledgeGapSourceTemplate;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

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

    fn fixtures() -> (TypedKnowledge, KnowledgeFrontier, AutonomousGapCycleRequest) {
        let records = vec![EvidenceRecord {
            evidence_id: "egfr-paper".into(),
            source_artifact: artifact("egfr-artifact"),
            source_kind: EvidenceSourceKind::Literature,
            claim: "EGFR signaling increases invasion".into(),
            scope: "organoid invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 800,
            release_epoch: 2,
        }];
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "close glioma evidence debt".into(),
                required_modalities: [GliomaModality::Genomics, GliomaModality::Imaging]
                    .into_iter()
                    .collect(),
                required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
                min_support_milli: 100,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &records,
        )
        .unwrap();
        let frontier = prioritize_knowledge_frontier(
            &KnowledgeFrontierRequest {
                objective: "close glioma evidence debt".into(),
                max_selected_claims: 4,
                min_priority_milli: 0,
                weights: Default::default(),
            },
            &knowledge,
        )
        .unwrap();
        let gap = KnowledgeGapCompilerRequest {
            objective: "close glioma evidence debt".into(),
            max_claims: 8,
            max_candidates: 16,
            max_candidates_per_claim: 8,
            max_template_cost_units: 10,
            min_frontier_priority_milli: 0,
            templates: vec![
                KnowledgeGapSourceTemplate {
                    template_id: "imaging-atlas".into(),
                    source_family: "atlas".into(),
                    source_kind: EvidenceAcquisitionSourceKind::Dataset,
                    modality: Some(GliomaModality::Imaging),
                    model_system: Some(GliomaModelSystem::Organoid),
                    cost_units: 2,
                    reproducibility_milli: 800,
                    failure_probability_milli: 100,
                    privacy_risk_milli: 50,
                    local_only: true,
                    contains_human_data: false,
                },
                KnowledgeGapSourceTemplate {
                    template_id: "replication-site".into(),
                    source_family: "consortium".into(),
                    source_kind: EvidenceAcquisitionSourceKind::Replication,
                    modality: None,
                    model_system: None,
                    cost_units: 3,
                    reproducibility_milli: 900,
                    failure_probability_milli: 150,
                    privacy_risk_milli: 50,
                    local_only: true,
                    contains_human_data: false,
                },
            ],
        };
        let planning = EvidenceAcquisitionRequest {
            objective: "close glioma evidence debt".into(),
            budget_units: 12,
            max_candidates: 16,
            max_selected: 4,
            beam_width: 16,
            min_source_families: 1,
            max_per_independence_group: 2,
            max_privacy_risk_milli: 1_000,
            min_portfolio_score_milli: 0,
            required_modalities: [GliomaModality::Imaging].into_iter().collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            weights: Default::default(),
        };
        (
            knowledge,
            frontier,
            AutonomousGapCycleRequest {
                gap,
                planning,
                execution_budget_units: 12,
                execution_max_retries: 2,
                stop_on_negative: false,
                require_artifacts: true,
            },
        )
    }

    #[test]
    fn cycle_compiles_plans_and_executes_a_local_dry_run() {
        let (knowledge, frontier, request) = fixtures();
        let mut executor = DryRunEvidenceAcquisitionExecutor;
        let output =
            execute_glioma_autonomous_gap_cycle(&request, &knowledge, &frontier, &mut executor)
                .unwrap();
        assert_eq!(output.phase_order.len(), 3);
        assert!(!output.gap_portfolio.candidates.is_empty());
        assert!(!output.acquisition_plan.selected_order.is_empty());
        assert!(output.simulation_only);
        assert_eq!(
            output.campaign.results.len(),
            output.campaign.execution_order.len()
        );
        output.validate().unwrap();
    }

    #[test]
    fn cycle_refuses_objective_drift_before_execution() {
        let (knowledge, frontier, mut request) = fixtures();
        request.planning.objective = "different objective".into();
        let mut executor = DryRunEvidenceAcquisitionExecutor;
        let error =
            execute_glioma_autonomous_gap_cycle(&request, &knowledge, &frontier, &mut executor)
                .unwrap_err();
        assert!(matches!(error, AutonomousGapCycleError::InvalidRequest(_)));
    }
}
