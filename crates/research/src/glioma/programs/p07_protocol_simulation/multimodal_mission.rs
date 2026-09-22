//! Multimodal/model-system portfolio expansion for autonomous glioma missions.
//!
//! The intent compiler creates one representative action per stage.  This feature expands that
//! graph into a bounded portfolio of modality/model-system variants so the autonomous controller
//! can trade information gain against cross-modal coverage, model diversity, cost, and
//! reproducibility.  Every variant still depends on the same canonical upstream handoff; no
//! downstream assay can run on an unqualified stage, and the portfolio remains preclinical and
//! institution-local.

use super::action_execution::GliomaActionExecutor;
use super::intent_mission::compile_glioma_intent_mission_candidates;
use super::mission::GliomaMissionGates;
use super::mission_recovery::{
    execute_glioma_mission_recovery, GliomaMissionRecovery, GliomaMissionRecoveryError,
    GliomaMissionRecoveryRequest,
};
use crate::glioma_engine::{
    compile_glioma_research, GliomaActionCandidate, GliomaEngineError, GliomaModality,
    GliomaModelSystem, GliomaPlanDisposition, GliomaResearchIntent, GliomaResearchPlan,
    GliomaSelectionConfig, GliomaStageKind, StageReadiness,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalResearchMission1@1";
pub const MAX_VARIANTS_PER_STAGE: u16 = 32;
pub const MAX_CANDIDATES: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaMultimodalMissionRequest {
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
    pub max_variants_per_stage: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMultimodalMissionDisposition {
    Executed,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaMultimodalResearchMission {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub mission_id: String,
    pub objective: String,
    pub plan: GliomaResearchPlan,
    pub candidate_order: Vec<String>,
    pub canonical_stage_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub model_system_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub campaign: Option<GliomaMissionRecovery>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaMultimodalMissionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaMultimodalMissionError {
    #[error("glioma multimodal mission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma multimodal intent compilation failed: {0}")]
    Compilation(#[from] GliomaEngineError),
    #[error("glioma multimodal mission recovery failed: {0}")]
    Recovery(#[from] GliomaMissionRecoveryError),
    #[error("glioma multimodal mission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma multimodal mission digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaMultimodalResearchMission) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "plan": output.plan,
        "candidate_order": output.candidate_order,
        "canonical_stage_order": output.canonical_stage_order,
        "modality_order": output.modality_order,
        "model_system_order": output.model_system_order,
        "omission_order": output.omission_order,
        "campaign": output.campaign,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn modality_label(value: GliomaModality) -> &'static str {
    match value {
        GliomaModality::Literature => "literature",
        GliomaModality::Histopathology => "histopathology",
        GliomaModality::Genomics => "genomics",
        GliomaModality::Transcriptomics => "transcriptomics",
        GliomaModality::Epigenomics => "epigenomics",
        GliomaModality::Proteomics => "proteomics",
        GliomaModality::Imaging => "imaging",
        GliomaModality::SingleCell => "single_cell",
        GliomaModality::Spatial => "spatial",
        GliomaModality::FunctionalPerturbation => "functional_perturbation",
        GliomaModality::OrganoidAssay => "organoid_assay",
        GliomaModality::AnimalModel => "animal_model",
        GliomaModality::Computational => "computational",
        GliomaModality::Instrument => "instrument",
        GliomaModality::Replication => "replication",
    }
}

fn model_label(value: GliomaModelSystem) -> &'static str {
    match value {
        GliomaModelSystem::CellLine => "cell_line",
        GliomaModelSystem::Organoid => "organoid",
        GliomaModelSystem::PatientDerivedXenograft => "patient_derived_xenograft",
        GliomaModelSystem::MouseModel => "mouse_model",
        GliomaModelSystem::ZebrafishModel => "zebrafish_model",
        GliomaModelSystem::InSilico => "in_silico",
    }
}

fn preferred_modalities(
    kind: GliomaStageKind,
    intent: &GliomaResearchIntent,
) -> Vec<GliomaModality> {
    let mut values = intent.modalities.iter().copied().collect::<Vec<_>>();
    values.retain(|modality| match kind {
        GliomaStageKind::EvidenceSurveillance | GliomaStageKind::EvidenceCompilation => {
            *modality == GliomaModality::Literature
        }
        GliomaStageKind::MolecularLandscape | GliomaStageKind::MechanismExploration => {
            modality.is_molecular() || *modality == GliomaModality::Literature
        }
        GliomaStageKind::ExperimentDesign | GliomaStageKind::ProtocolSimulation => matches!(
            modality,
            GliomaModality::FunctionalPerturbation
                | GliomaModality::OrganoidAssay
                | GliomaModality::AnimalModel
                | GliomaModality::Imaging
        ),
        GliomaStageKind::InstrumentPreflight => *modality == GliomaModality::Instrument,
        GliomaStageKind::ComputationalExecution | GliomaStageKind::StatisticalInterpretation => {
            *modality == GliomaModality::Computational || modality.is_molecular()
        }
        GliomaStageKind::ReplicationRobustness => {
            *modality == GliomaModality::Replication || modality.is_molecular()
        }
        GliomaStageKind::IntentNormalization
        | GliomaStageKind::MultimodalIngestionQc
        | GliomaStageKind::ResearchObjectRelease
        | GliomaStageKind::FederationBenchmarking => true,
    });
    if values.is_empty() {
        values = intent.modalities.iter().copied().collect();
    }
    if values.is_empty() {
        values.push(GliomaModality::Computational);
    }
    values
}

fn preferred_models(
    kind: GliomaStageKind,
    intent: &GliomaResearchIntent,
) -> Vec<GliomaModelSystem> {
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
        return vec![GliomaModelSystem::InSilico];
    }
    let mut values = intent.model_systems.iter().copied().collect::<Vec<_>>();
    if values.is_empty() {
        values.push(GliomaModelSystem::InSilico);
    }
    values
}

fn score_with_diversity(score: u16, index: usize, stride: u16) -> u16 {
    score
        .saturating_add((index as u16).saturating_mul(stride))
        .min(1_000)
}

/// Expand the admitted stage graph into a bounded modality/model-system portfolio.
pub fn compile_glioma_multimodal_mission_candidates(
    intent: &GliomaResearchIntent,
    plan: &GliomaResearchPlan,
    max_variants_per_stage: u16,
) -> Result<Vec<GliomaActionCandidate>, GliomaMultimodalMissionError> {
    if max_variants_per_stage == 0 || max_variants_per_stage > MAX_VARIANTS_PER_STAGE {
        return Err(GliomaMultimodalMissionError::InvalidRequest(
            "max_variants_per_stage must be within the bounded multimodal portfolio limit".into(),
        ));
    }
    let canonical = compile_glioma_intent_mission_candidates(intent, plan);
    let canonical_by_stage = canonical
        .iter()
        .map(|candidate| (candidate.stage_kind, candidate))
        .collect::<BTreeMap<_, _>>();
    let enabled = plan
        .stages
        .iter()
        .filter(|stage| stage.required && stage.readiness == StageReadiness::Ready)
        .map(|stage| stage.stage_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for stage in &plan.stages {
        if !stage.required || stage.readiness != StageReadiness::Ready {
            continue;
        }
        let base = canonical_by_stage.get(&stage.kind).ok_or_else(|| {
            GliomaMultimodalMissionError::InvalidOutput(format!(
                "admitted stage {} has no canonical action",
                stage.stage_id
            ))
        })?;
        let modalities = preferred_modalities(stage.kind, intent);
        let models = preferred_models(stage.kind, intent);
        let mut variants = Vec::new();
        for (modality_index, modality) in modalities.iter().enumerate() {
            for (model_index, model_system) in models.iter().enumerate() {
                if variants.len() >= usize::from(max_variants_per_stage) {
                    break;
                }
                variants.push((modality_index, model_index, *modality, *model_system));
            }
            if variants.len() >= usize::from(max_variants_per_stage) {
                break;
            }
        }
        for (modality_index, model_index, modality, model_system) in variants {
            let action_id = format!(
                "glioma-stage:{}:m{}:s{}",
                stage.stage_id,
                modality_label(modality),
                model_label(model_system)
            );
            let depends_on = stage
                .depends_on
                .iter()
                .filter(|dependency| enabled.contains(dependency.as_str()))
                .map(|dependency| {
                    let dependency_stage = plan
                        .stages
                        .iter()
                        .find(|candidate| candidate.stage_id == *dependency)
                        .expect("validated plan dependency must exist");
                    let dependency_base = canonical_by_stage
                        .get(&dependency_stage.kind)
                        .expect("validated plan dependency must have canonical action");
                    format!(
                        "glioma-stage:{}:m{}:s{}",
                        dependency,
                        modality_label(dependency_base.modality),
                        model_label(dependency_base.model_system)
                    )
                })
                .collect();
            candidates.push(GliomaActionCandidate {
                action_id,
                stage_kind: stage.kind,
                modality,
                model_system,
                depends_on,
                cost_units: base.cost_units,
                information_gain_milli: score_with_diversity(
                    base.information_gain_milli,
                    modality_index + model_index,
                    12,
                ),
                frontier_novelty_milli: score_with_diversity(
                    base.frontier_novelty_milli,
                    modality_index + model_index,
                    16,
                ),
                workflow_leverage_milli: base.workflow_leverage_milli,
                cross_stage_unlock_milli: base.cross_stage_unlock_milli,
                reproducibility_safety_milli: base.reproducibility_safety_milli,
                federation_value_milli: base.federation_value_milli,
                feasibility_milli: base.feasibility_milli,
                autonomy_tier: base.autonomy_tier,
                effects: base.effects.clone(),
            });
        }
    }
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    if candidates.len() > MAX_CANDIDATES {
        return Err(GliomaMultimodalMissionError::InvalidRequest(
            "multimodal candidate portfolio exceeds the bounded candidate limit".into(),
        ));
    }
    Ok(candidates)
}

/// Compile, expand, and execute a multimodal/model-system glioma research mission.
pub fn execute_glioma_multimodal_mission<E: GliomaActionExecutor>(
    request: &GliomaMultimodalMissionRequest,
    executor: &mut E,
) -> Result<GliomaMultimodalResearchMission, GliomaMultimodalMissionError> {
    if request.mission_id.trim().is_empty()
        || request.max_rounds == 0
        || request.recovery_budget_units == 0
        || request.recovery_max_rounds == 0
    {
        return Err(GliomaMultimodalMissionError::InvalidRequest(
            "mission identity, initial rounds, recovery budget, and recovery rounds are required"
                .into(),
        ));
    }
    let plan = compile_glioma_research(&request.intent)?;
    let candidates = compile_glioma_multimodal_mission_candidates(
        &request.intent,
        &plan,
        request.max_variants_per_stage,
    )?;
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let mut canonical_stage_order = plan
        .stages
        .iter()
        .filter(|stage| stage.required && stage.readiness == StageReadiness::Ready)
        .map(|stage| stage.stage_id.clone())
        .collect::<Vec<_>>();
    canonical_stage_order.sort();
    let modality_order = candidates
        .iter()
        .map(|candidate| modality_label(candidate.modality).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let model_system_order = candidates
        .iter()
        .map(|candidate| model_label(candidate.model_system).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = match plan.disposition {
        GliomaPlanDisposition::NeedsInputs | GliomaPlanDisposition::ApprovalRequired => {
            GliomaMultimodalMissionDisposition::Held
        }
        GliomaPlanDisposition::Blocked => GliomaMultimodalMissionDisposition::Blocked,
        GliomaPlanDisposition::Admitted => GliomaMultimodalMissionDisposition::Executed,
    };
    let campaign = if plan.disposition == GliomaPlanDisposition::Admitted {
        let mission_request = super::mission::GliomaMissionRequest {
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
    let omission_order = plan.omission_order.clone();
    let (negative_evidence, uncertainty) = campaign
        .as_ref()
        .map(|campaign| {
            (
                campaign.negative_evidence.clone(),
                campaign.uncertainty.clone(),
            )
        })
        .unwrap_or_else(|| (Vec::new(), omission_order.clone()));
    let mut output = GliomaMultimodalResearchMission {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.intent.research_id.clone(),
        study_id: request.intent.study_id.clone(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        plan,
        candidate_order,
        canonical_stage_order,
        modality_order,
        model_system_order,
        omission_order,
        campaign,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-mission"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaMultimodalMissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl GliomaMultimodalResearchMission {
    pub fn validate(&self) -> Result<(), GliomaMultimodalMissionError> {
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
            || !canonical(&self.canonical_stage_order)
            || !canonical(&self.modality_order)
            || !canonical(&self.model_system_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.campaign.as_ref().is_some_and(|campaign| {
                campaign.mission_id != self.mission_id || campaign.objective != self.objective
            })
        {
            return Err(GliomaMultimodalMissionError::InvalidOutput(
                "multimodal mission identity, plan binding, ordering, or campaign fields are invalid"
                    .into(),
            ));
        }
        self.plan.validate()?;
        if let Some(campaign) = &self.campaign {
            campaign.validate()?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaMultimodalMissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaMultimodalMissionError::InvalidOutput(
                "multimodal mission digest is not content-addressed".into(),
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

    fn request(with_inputs: bool) -> GliomaMultimodalMissionRequest {
        GliomaMultimodalMissionRequest {
            intent: GliomaResearchIntent {
                research_id: "multimodal-mission-research".into(),
                study_id: "multimodal-mission-study".into(),
                objective: "map invasion mechanisms across organoid and cell-line systems".into(),
                output_uses: BTreeSet::from([bioprism_onco::OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([
                    GliomaModelSystem::Organoid,
                    GliomaModelSystem::CellLine,
                ]),
                modalities: BTreeSet::from([
                    GliomaModality::Literature,
                    GliomaModality::Transcriptomics,
                    GliomaModality::Imaging,
                    GliomaModality::Computational,
                    GliomaModality::OrganoidAssay,
                    GliomaModality::Replication,
                ]),
                input_artifacts: if with_inputs {
                    vec![crate::glioma_engine::LocalArtifactRef {
                        artifact_id: "multimodal-input".into(),
                        content_hash: ContentHash::of_bytes(b"multimodal-input"),
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
                budget_units: 2_000,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: ContentHash::of_bytes(b"multimodal-replay"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            mission_id: "multimodal-mission".into(),
            selection: GliomaSelectionConfig {
                budget_units: 2_000,
                max_actions: 32,
                ..GliomaSelectionConfig::default()
            },
            gates: GliomaMissionGates {
                required_stages: BTreeSet::from([GliomaStageKind::MechanismExploration]),
                min_completed_actions: 2,
                min_information_gain_milli: 500,
                max_uncertainty_milli: 20_000,
                min_model_systems: 2,
                min_modalities: 3,
            },
            max_rounds: 16,
            max_retries: 1,
            require_artifacts: true,
            stop_on_negative: false,
            recovery_budget_units: 512,
            recovery_max_rounds: 4,
            require_clean_recovery: false,
            max_variants_per_stage: 8,
        }
    }

    #[test]
    fn multimodal_compiler_expands_coverage_and_replays() {
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_multimodal_mission(&request(true), &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            GliomaMultimodalMissionDisposition::Executed
        );
        assert!(output.candidate_order.len() > output.canonical_stage_order.len());
        assert!(output.modality_order.len() >= 3);
        assert!(output.model_system_order.len() >= 2);
        assert!(output.campaign.is_some());
        output.validate().unwrap();
    }

    #[test]
    fn multimodal_missing_inputs_hold_without_dispatch() {
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_multimodal_mission(&request(false), &mut executor).unwrap();
        assert_eq!(output.disposition, GliomaMultimodalMissionDisposition::Held);
        assert!(output.campaign.is_none());
        assert!(!output.omission_order.is_empty());
    }
}
