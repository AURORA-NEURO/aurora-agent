//! Intent-to-evidence operating cycle for preclinical glioma research.
//!
//! P01 already separates portfolio selection from local acquisition. This feature makes the
//! researcher-facing path executable in one bounded capability: score a source-diverse evidence
//! portfolio, execute it through an institution-owned adapter, and return an honest continuation
//! frontier. Synthetic acquisition is explicitly unknown and negative; it never becomes support.

use super::acquisition::{
    plan_glioma_evidence_acquisition, EvidenceAcquisitionCandidate, EvidenceAcquisitionError,
    EvidenceAcquisitionPlan, EvidenceAcquisitionRequest,
};
use super::acquisition_campaign::{
    execute_glioma_evidence_acquisition_campaign, DryRunEvidenceAcquisitionExecutor,
    EvidenceAcquisitionCampaign, EvidenceAcquisitionCampaignDisposition,
    EvidenceAcquisitionCampaignError, EvidenceAcquisitionCampaignRequest,
    EvidenceAcquisitionExecutor,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEvidenceOperatingCycleRequest {
    pub planning: EvidenceAcquisitionRequest,
    pub candidates: Vec<EvidenceAcquisitionCandidate>,
    pub max_retries: u8,
    pub stop_on_negative: bool,
    pub require_artifacts: bool,
    pub execution_mode: EvidenceExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEvidenceOperatingCycleDisposition {
    Ready,
    Partial,
    Negative,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEvidenceOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub plan: EvidenceAcquisitionPlan,
    pub campaign: EvidenceAcquisitionCampaign,
    pub simulation_only: bool,
    pub execution_mode: EvidenceExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaEvidenceOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEvidenceOperatingCycleError {
    #[error("evidence operating-cycle planning failed: {0}")]
    Planning(#[from] EvidenceAcquisitionError),
    #[error("evidence operating-cycle campaign failed: {0}")]
    Campaign(#[from] EvidenceAcquisitionCampaignError),
    #[error("evidence operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaEvidenceOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "plan": output.plan,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn disposition(campaign: &EvidenceAcquisitionCampaign) -> GliomaEvidenceOperatingCycleDisposition {
    match campaign.disposition {
        EvidenceAcquisitionCampaignDisposition::Completed
            if campaign.negative_evidence.is_empty() =>
        {
            GliomaEvidenceOperatingCycleDisposition::Ready
        }
        EvidenceAcquisitionCampaignDisposition::Completed
        | EvidenceAcquisitionCampaignDisposition::Negative => {
            GliomaEvidenceOperatingCycleDisposition::Negative
        }
        EvidenceAcquisitionCampaignDisposition::Partial => {
            GliomaEvidenceOperatingCycleDisposition::Partial
        }
        EvidenceAcquisitionCampaignDisposition::Blocked
        | EvidenceAcquisitionCampaignDisposition::Failed => {
            GliomaEvidenceOperatingCycleDisposition::Blocked
        }
        EvidenceAcquisitionCampaignDisposition::Unresolved => {
            GliomaEvidenceOperatingCycleDisposition::Unresolved
        }
    }
}

impl GliomaEvidenceOperatingCycle {
    pub fn validate(&self) -> Result<(), GliomaEvidenceOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "portfolio_selection".to_string(),
                    "local_acquisition".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.plan.objective != self.objective
            || self.campaign.objective != self.objective
            || self.campaign.plan_digest != self.plan.digest
            || self.simulation_only
                != matches!(self.execution_mode, EvidenceExecutionMode::LocalSimulation)
        {
            return Err(GliomaEvidenceOperatingCycleError::InvalidOutput(
                "identity, phases, plan/campaign binding, evidence ordering, or execution mode is invalid".into(),
            ));
        }
        self.plan
            .validate()
            .map_err(|error| GliomaEvidenceOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.campaign
            .validate()
            .map_err(|error| GliomaEvidenceOperatingCycleError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaEvidenceOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEvidenceOperatingCycleError::InvalidOutput(
                "evidence operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Plan a source-diverse portfolio and execute it through a caller-owned evidence adapter.
pub fn execute_glioma_evidence_operating_cycle<E: EvidenceAcquisitionExecutor>(
    request: &GliomaEvidenceOperatingCycleRequest,
    executor: &mut E,
) -> Result<GliomaEvidenceOperatingCycle, GliomaEvidenceOperatingCycleError> {
    let plan = plan_glioma_evidence_acquisition(&request.planning, &request.candidates)?;
    let campaign = execute_glioma_evidence_acquisition_campaign(
        &EvidenceAcquisitionCampaignRequest {
            objective: request.planning.objective.clone(),
            plan: plan.clone(),
            candidates: request.candidates.clone(),
            budget_units: request.planning.budget_units,
            max_retries: request.max_retries,
            stop_on_negative: request.stop_on_negative,
            require_artifacts: request.require_artifacts,
        },
        executor,
    )?;
    let cycle_disposition = disposition(&campaign);
    let mut negative_evidence = plan.negative_evidence.clone();
    negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = plan.uncertainty.clone();
    uncertainty.extend(campaign.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = match cycle_disposition {
        GliomaEvidenceOperatingCycleDisposition::Ready => {
            "compile the qualified local evidence into typed knowledge and retain source lineage".into()
        }
        GliomaEvidenceOperatingCycleDisposition::Partial => {
            "review partial acquisition outcomes and schedule the highest-value missing source family".into()
        }
        GliomaEvidenceOperatingCycleDisposition::Negative => {
            "preserve the negative evidence and route contradictory or null claims to replication review".into()
        }
        GliomaEvidenceOperatingCycleDisposition::Blocked => {
            "resolve policy, dependency, adapter, or budget blockers before retrying acquisition".into()
        }
        GliomaEvidenceOperatingCycleDisposition::Unresolved => {
            "obtain local source results before treating the evidence frontier as qualified".into()
        }
    };
    let mut output = GliomaEvidenceOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.planning.objective.clone(),
        phase_order: vec![
            "portfolio_selection".into(),
            "local_acquisition".into(),
            "operator_handoff".into(),
        ],
        plan,
        simulation_only: matches!(
            request.execution_mode,
            EvidenceExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        campaign,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition: cycle_disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaEvidenceOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_evidence_operating_cycle_dry_run(
    request: &GliomaEvidenceOperatingCycleRequest,
) -> Result<GliomaEvidenceOperatingCycle, GliomaEvidenceOperatingCycleError> {
    let mut executor = DryRunEvidenceAcquisitionExecutor;
    execute_glioma_evidence_operating_cycle(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
    use std::collections::BTreeSet;

    fn request() -> GliomaEvidenceOperatingCycleRequest {
        GliomaEvidenceOperatingCycleRequest {
            planning: EvidenceAcquisitionRequest {
                objective: "map reproducible preclinical glioma invasion evidence".into(),
                budget_units: 2,
                max_candidates: 8,
                max_selected: 1,
                beam_width: 8,
                min_source_families: 1,
                max_per_independence_group: 1,
                max_privacy_risk_milli: 200,
                min_portfolio_score_milli: 0,
                required_modalities: BTreeSet::from([GliomaModality::Literature]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                weights: Default::default(),
            },
            candidates: vec![EvidenceAcquisitionCandidate {
                candidate_id: "literature-invasion".into(),
                target_claim: "invasion program is reproducible".into(),
                source_family: "preclinical-literature".into(),
                source_kind: super::super::acquisition::EvidenceAcquisitionSourceKind::Literature,
                modality: GliomaModality::Literature,
                model_system: Some(GliomaModelSystem::Organoid),
                independence_group: "source-a".into(),
                depends_on: Vec::new(),
                cost_units: 1,
                expected_support_milli: 800,
                expected_uncertainty_reduction_milli: 700,
                contradiction_resolution_milli: 500,
                freshness_milli: 800,
                workflow_leverage_milli: 700,
                reproducibility_milli: 800,
                failure_probability_milli: 50,
                privacy_risk_milli: 10,
                local_only: true,
                contains_human_data: false,
            }],
            max_retries: 1,
            stop_on_negative: false,
            require_artifacts: true,
            execution_mode: EvidenceExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn operating_cycle_plans_executes_and_preserves_unknown_dry_run() {
        let first = execute_glioma_evidence_operating_cycle_dry_run(&request()).unwrap();
        let second = execute_glioma_evidence_operating_cycle_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert!(first.simulation_only);
        assert_eq!(first.campaign.unknown_order, vec!["literature-invasion"]);
        assert_eq!(first.phase_order[0], "portfolio_selection");
        first.validate().unwrap();
    }
}
