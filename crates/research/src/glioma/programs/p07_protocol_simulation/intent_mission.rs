//! Intent-to-mission compilation for autonomous preclinical glioma research.
//!
//! This is the product seam between a researcher's bounded glioma intent and the adaptive
//! mission controller.  It derives one typed action per admitted research stage, closes stage
//! dependencies, selects representative modality/model-system bindings, and then delegates
//! execution and recovery to the existing science-aware controllers.  Missing evidence, unsafe
//! locality, or required approvals remain a held plan; they are never silently converted into a
//! runnable action graph.

use super::action_execution::GliomaActionExecutor;
use super::mission::{GliomaMissionGates, GliomaMissionRequest};
use super::mission_recovery::{
    execute_glioma_mission_recovery, GliomaMissionRecovery, GliomaMissionRecoveryError,
    GliomaMissionRecoveryRequest,
};
use crate::glioma_engine::{
    compile_glioma_research, GliomaActionCandidate, GliomaEngineError, GliomaModality,
    GliomaModelSystem, GliomaPlanDisposition, GliomaResearchIntent, GliomaResearchPlan,
    GliomaSelectionConfig, GliomaStageKind,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaIntentResearchMission1@1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaIntentMissionRequest {
    pub intent: GliomaResearchIntent,
    pub mission_id: String,
    pub selection: GliomaSelectionConfig,
    pub gates: GliomaMissionGates,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_negative: bool,
    pub recovery_budget_units: u32,
    pub recovery_max_rounds: u16,
    pub require_clean_recovery: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaIntentMissionDisposition {
    Executed,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaIntentResearchMission {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub mission_id: String,
    pub objective: String,
    pub plan: GliomaResearchPlan,
    pub candidate_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub campaign: Option<GliomaMissionRecovery>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaIntentMissionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaIntentMissionError {
    #[error("glioma intent mission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma intent compilation failed: {0}")]
    Compilation(#[from] GliomaEngineError),
    #[error("glioma mission recovery failed: {0}")]
    Recovery(#[from] GliomaMissionRecoveryError),
    #[error("glioma intent mission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma intent mission digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaIntentResearchMission) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "plan": output.plan,
        "candidate_order": output.candidate_order,
        "omission_order": output.omission_order,
        "campaign": output.campaign,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn representative_modality(kind: GliomaStageKind, intent: &GliomaResearchIntent) -> GliomaModality {
    let preferred = match kind {
        GliomaStageKind::EvidenceSurveillance | GliomaStageKind::EvidenceCompilation => {
            GliomaModality::Literature
        }
        GliomaStageKind::MolecularLandscape | GliomaStageKind::MechanismExploration => intent
            .modalities
            .iter()
            .copied()
            .find(|modality| modality.is_molecular())
            .unwrap_or(GliomaModality::Computational),
        GliomaStageKind::InstrumentPreflight => GliomaModality::Instrument,
        GliomaStageKind::ComputationalExecution | GliomaStageKind::StatisticalInterpretation => {
            GliomaModality::Computational
        }
        GliomaStageKind::ReplicationRobustness => GliomaModality::Replication,
        GliomaStageKind::MultimodalIngestionQc => intent
            .modalities
            .iter()
            .copied()
            .next()
            .unwrap_or(GliomaModality::Computational),
        GliomaStageKind::ExperimentDesign | GliomaStageKind::ProtocolSimulation => intent
            .modalities
            .iter()
            .copied()
            .find(|modality| {
                matches!(
                    modality,
                    GliomaModality::OrganoidAssay
                        | GliomaModality::FunctionalPerturbation
                        | GliomaModality::AnimalModel
                )
            })
            .unwrap_or(GliomaModality::OrganoidAssay),
        GliomaStageKind::IntentNormalization | GliomaStageKind::ResearchObjectRelease => {
            GliomaModality::Computational
        }
        GliomaStageKind::FederationBenchmarking => GliomaModality::Replication,
    };
    if intent.modalities.contains(&preferred) {
        preferred
    } else {
        intent
            .modalities
            .iter()
            .copied()
            .next()
            .unwrap_or(GliomaModality::Computational)
    }
}

fn representative_model(kind: GliomaStageKind, intent: &GliomaResearchIntent) -> GliomaModelSystem {
    if matches!(
        kind,
        GliomaStageKind::IntentNormalization
            | GliomaStageKind::EvidenceSurveillance
            | GliomaStageKind::EvidenceCompilation
            | GliomaStageKind::ComputationalExecution
            | GliomaStageKind::StatisticalInterpretation
            | GliomaStageKind::ResearchObjectRelease
            | GliomaStageKind::FederationBenchmarking
    ) {
        GliomaModelSystem::InSilico
    } else {
        intent
            .model_systems
            .iter()
            .copied()
            .next()
            .unwrap_or(GliomaModelSystem::InSilico)
    }
}

fn stage_score(kind: GliomaStageKind) -> (u16, u16, u16, u16, u16, u16, u16) {
    match kind {
        GliomaStageKind::IntentNormalization => (500, 350, 700, 900, 900, 200, 950),
        GliomaStageKind::EvidenceSurveillance => (800, 700, 750, 700, 800, 500, 850),
        GliomaStageKind::EvidenceCompilation => (820, 720, 800, 750, 850, 500, 820),
        GliomaStageKind::MultimodalIngestionQc => (780, 650, 900, 820, 950, 500, 800),
        GliomaStageKind::MolecularLandscape => (900, 850, 850, 880, 800, 600, 700),
        GliomaStageKind::MechanismExploration => (950, 950, 900, 950, 850, 650, 700),
        GliomaStageKind::ExperimentDesign => (880, 820, 900, 900, 900, 550, 760),
        GliomaStageKind::ProtocolSimulation => (760, 700, 950, 880, 950, 500, 900),
        GliomaStageKind::InstrumentPreflight => (600, 650, 900, 760, 1_000, 450, 700),
        GliomaStageKind::ComputationalExecution => (900, 820, 920, 900, 980, 650, 780),
        GliomaStageKind::StatisticalInterpretation => (920, 850, 880, 900, 900, 700, 760),
        GliomaStageKind::ReplicationRobustness => (930, 900, 850, 920, 980, 800, 650),
        GliomaStageKind::ResearchObjectRelease => (650, 700, 800, 850, 1_000, 900, 900),
        GliomaStageKind::FederationBenchmarking => (700, 780, 800, 900, 950, 1_000, 600),
    }
}

/// Compile the admitted stages of a glioma intent into typed mission candidates.
pub fn compile_glioma_intent_mission_candidates(
    intent: &GliomaResearchIntent,
    plan: &GliomaResearchPlan,
) -> Vec<GliomaActionCandidate> {
    let enabled = plan
        .stages
        .iter()
        .filter(|stage| {
            stage.required && stage.readiness == crate::glioma_engine::StageReadiness::Ready
        })
        .map(|stage| stage.stage_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut candidates = plan
        .stages
        .iter()
        .filter(|stage| enabled.contains(stage.stage_id.as_str()))
        .map(|stage| {
            let action_id = format!("glioma-stage:{}", stage.stage_id);
            let depends_on = stage
                .depends_on
                .iter()
                .filter(|dependency| enabled.contains(dependency.as_str()))
                .map(|dependency| format!("glioma-stage:{dependency}"))
                .collect();
            let (
                information_gain_milli,
                frontier_novelty_milli,
                workflow_leverage_milli,
                cross_stage_unlock_milli,
                reproducibility_safety_milli,
                federation_value_milli,
                feasibility_milli,
            ) = stage_score(stage.kind);
            GliomaActionCandidate {
                action_id,
                stage_kind: stage.kind,
                modality: representative_modality(stage.kind, intent),
                model_system: representative_model(stage.kind, intent),
                depends_on,
                cost_units: stage.budget_units.max(1),
                information_gain_milli,
                frontier_novelty_milli,
                workflow_leverage_milli,
                cross_stage_unlock_milli,
                reproducibility_safety_milli,
                federation_value_milli,
                feasibility_milli,
                autonomy_tier: stage.autonomy_tier,
                effects: stage.effects.clone(),
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    candidates
}

/// Compile an intent, derive the stage action graph, and execute it through mission recovery.
pub fn execute_glioma_intent_mission<E: GliomaActionExecutor>(
    request: &GliomaIntentMissionRequest,
    executor: &mut E,
) -> Result<GliomaIntentResearchMission, GliomaIntentMissionError> {
    if request.mission_id.trim().is_empty()
        || request.max_rounds == 0
        || request.recovery_budget_units == 0
        || request.recovery_max_rounds == 0
    {
        return Err(GliomaIntentMissionError::InvalidRequest(
            "mission identity, initial rounds, recovery budget, and recovery rounds are required"
                .into(),
        ));
    }
    let plan = compile_glioma_research(&request.intent)?;
    let candidates = compile_glioma_intent_mission_candidates(&request.intent, &plan);
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = match plan.disposition {
        GliomaPlanDisposition::NeedsInputs | GliomaPlanDisposition::ApprovalRequired => {
            GliomaIntentMissionDisposition::Held
        }
        GliomaPlanDisposition::Blocked => GliomaIntentMissionDisposition::Blocked,
        GliomaPlanDisposition::Admitted => GliomaIntentMissionDisposition::Executed,
    };
    let campaign = if plan.disposition == GliomaPlanDisposition::Admitted {
        if candidates.is_empty() {
            return Err(GliomaIntentMissionError::InvalidRequest(
                "an admitted intent must compile at least one runnable stage candidate".into(),
            ));
        }
        let mission_request = GliomaMissionRequest {
            mission_id: request.mission_id.clone(),
            objective: request.intent.objective.clone(),
            candidates,
            completed_action_order: Vec::new(),
            selection: request.selection.clone(),
            gates: request.gates.clone(),
            max_rounds: request.max_rounds,
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
            stop_on_negative: request.stop_on_negative,
        };
        Some(execute_glioma_mission_recovery(
            &GliomaMissionRecoveryRequest {
                initial: mission_request,
                recovery_budget_units: request.recovery_budget_units,
                recovery_max_rounds: request.recovery_max_rounds,
                require_clean_qualification: request.require_clean_recovery,
            },
            executor,
        )?)
    } else {
        None
    };
    let plan_omissions = plan.omission_order.clone();
    let (negative_evidence, uncertainty) = campaign
        .as_ref()
        .map(|campaign| {
            (
                campaign.negative_evidence.clone(),
                campaign.uncertainty.clone(),
            )
        })
        .unwrap_or_else(|| (Vec::new(), plan_omissions.clone()));
    let mut output = GliomaIntentResearchMission {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.intent.research_id.clone(),
        study_id: request.intent.study_id.clone(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        plan,
        candidate_order,
        omission_order: plan_omissions,
        campaign,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-intent-mission"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaIntentMissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl GliomaIntentResearchMission {
    pub fn validate(&self) -> Result<(), GliomaIntentMissionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.plan.research_id != self.research_id
            || self.plan.study_id != self.study_id
            || self.plan.objective != self.objective
            || !canonical(&self.candidate_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.campaign.as_ref().is_some_and(|campaign| {
                campaign.mission_id != self.mission_id || campaign.objective != self.objective
            })
        {
            return Err(GliomaIntentMissionError::InvalidOutput(
                "intent, plan binding, candidate ordering, omission, or campaign fields are invalid"
                    .into(),
            ));
        }
        self.plan.validate()?;
        if let Some(campaign) = &self.campaign {
            campaign.validate()?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaIntentMissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaIntentMissionError::InvalidOutput(
                "intent mission digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::DryRunGliomaActionExecutor;
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};

    fn intent(with_inputs: bool) -> GliomaResearchIntent {
        GliomaResearchIntent {
            research_id: "intent-mission-research".into(),
            study_id: "intent-mission-study".into(),
            objective: "map reproducible invasion mechanisms in glioma organoids".into(),
            output_uses: BTreeSet::from([bioprism_onco::OutputUse::CohortAnalysis]),
            model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            modalities: BTreeSet::from([
                GliomaModality::Literature,
                GliomaModality::Genomics,
                GliomaModality::Computational,
                GliomaModality::Replication,
                GliomaModality::OrganoidAssay,
            ]),
            input_artifacts: if with_inputs {
                vec![crate::glioma_engine::LocalArtifactRef {
                    artifact_id: "local-glioma-object".into(),
                    content_hash: ContentHash::of_bytes(b"local-glioma-object"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }]
            } else {
                Vec::new()
            },
            requested_autonomy: AutonomyTier::A0,
            approval_reference: None,
            budget_units: 512,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: ContentHash::of_bytes(b"intent-mission-replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(with_inputs: bool) -> GliomaIntentMissionRequest {
        GliomaIntentMissionRequest {
            intent: intent(with_inputs),
            mission_id: "intent-mission".into(),
            selection: GliomaSelectionConfig {
                budget_units: 512,
                max_actions: 14,
                ..GliomaSelectionConfig::default()
            },
            gates: GliomaMissionGates {
                required_stages: BTreeSet::from([GliomaStageKind::MechanismExploration]),
                min_completed_actions: 1,
                min_information_gain_milli: 500,
                max_uncertainty_milli: 10_000,
                min_model_systems: 1,
                min_modalities: 1,
            },
            max_rounds: 14,
            max_retries: 1,
            require_artifacts: true,
            stop_on_negative: false,
            recovery_budget_units: 128,
            recovery_max_rounds: 4,
            require_clean_recovery: false,
        }
    }

    #[test]
    fn intent_compiles_stage_graph_and_executes_a_local_mission() {
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_intent_mission(&request(true), &mut executor).unwrap();
        assert_eq!(output.plan.disposition, GliomaPlanDisposition::Admitted);
        assert!(output.candidate_order.len() >= 10);
        assert!(output.campaign.is_some());
        output.validate().unwrap();
    }

    #[test]
    fn missing_input_intent_is_held_without_dispatch() {
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_intent_mission(&request(false), &mut executor).unwrap();
        assert_eq!(output.disposition, GliomaIntentMissionDisposition::Held);
        assert!(output.campaign.is_none());
        assert!(!output.omission_order.is_empty());
    }
}
