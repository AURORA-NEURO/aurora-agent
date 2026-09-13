//! Multimodal research-readiness admission after an ingestion/QC campaign.
//!
//! P03 produces more than a QC report: it must decide which downstream scientific workflows can
//! consume the current local observations. This gate quantifies modality/model coverage and
//! comparable-observation quality, then independently admits, conditions, or blocks analysis,
//! mechanism exploration, experiment design, replication, and publication. It never imputes a
//! missing modality, upgrades a partial report, or makes a clinical decision.

use super::campaign::{
    execute_glioma_multimodal_ingestion_campaign, MultimodalIngestionCampaign,
    MultimodalIngestionCampaignError, MultimodalIngestionCampaignExecutor,
    MultimodalIngestionCampaignRequest,
};
use crate::glioma::multimodal::{MultimodalDisposition, MultimodalQcReport};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalResearchReadiness1@1";
pub const MAX_SURFACES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalResearchSurface {
    Analysis,
    Mechanism,
    ExperimentDesign,
    Replication,
    Publication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalSurfaceReadiness {
    Admitted,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalSurfaceDecision {
    pub surface: MultimodalResearchSurface,
    pub readiness: MultimodalSurfaceReadiness,
    pub rationale: String,
    pub required_action_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalReadinessRequest {
    pub campaign: MultimodalIngestionCampaignRequest,
    pub min_comparable_observations: usize,
    pub min_coverage_milli: u16,
    pub min_quality_milli: u16,
    pub require_complete_report: bool,
    pub required_surfaces: BTreeSet<MultimodalResearchSurface>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalResearchReadinessDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalResearchReadiness {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub campaign_digest: ContentHash,
    pub final_report_digest: ContentHash,
    pub modality_coverage_milli: u16,
    pub model_coverage_milli: u16,
    pub coverage_milli: u16,
    pub quality_milli: u16,
    pub comparable_observation_count: usize,
    pub observation_count: usize,
    pub surface_order: Vec<MultimodalResearchSurface>,
    pub decisions: Vec<MultimodalSurfaceDecision>,
    pub admitted_surface_order: Vec<MultimodalResearchSurface>,
    pub conditional_surface_order: Vec<MultimodalResearchSurface>,
    pub blocked_surface_order: Vec<MultimodalResearchSurface>,
    pub unresolved_surface_order: Vec<MultimodalResearchSurface>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: MultimodalResearchReadinessDisposition,
    pub campaign: MultimodalIngestionCampaign,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalReadinessError {
    #[error("multimodal readiness request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal readiness campaign failed: {0}")]
    Campaign(#[from] MultimodalIngestionCampaignError),
    #[error("multimodal readiness output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal readiness digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalResearchReadiness) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "campaign_digest": output.campaign_digest,
        "final_report_digest": output.final_report_digest,
        "modality_coverage_milli": output.modality_coverage_milli,
        "model_coverage_milli": output.model_coverage_milli,
        "coverage_milli": output.coverage_milli,
        "quality_milli": output.quality_milli,
        "comparable_observation_count": output.comparable_observation_count,
        "observation_count": output.observation_count,
        "surface_order": output.surface_order,
        "decisions": output.decisions,
        "admitted_surface_order": output.admitted_surface_order,
        "conditional_surface_order": output.conditional_surface_order,
        "blocked_surface_order": output.blocked_surface_order,
        "unresolved_surface_order": output.unresolved_surface_order,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "campaign": output.campaign,
    })
}

impl MultimodalResearchReadiness {
    pub fn validate(&self) -> Result<(), MultimodalReadinessError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.campaign_digest.as_str().len() != 64
            || self.final_report_digest.as_str().len() != 64
            || self.modality_coverage_milli > 1_000
            || self.model_coverage_milli > 1_000
            || self.coverage_milli > 1_000
            || self.quality_milli > 1_000
            || !canonical(&self.surface_order)
            || self.surface_order.len() > MAX_SURFACES
            || self.decisions.len() != self.surface_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.surface)
                .collect::<Vec<_>>()
                != self.surface_order
            || !canonical(&self.admitted_surface_order)
            || !canonical(&self.conditional_surface_order)
            || !canonical(&self.blocked_surface_order)
            || !canonical(&self.unresolved_surface_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.decisions.iter().any(|decision| {
                decision.rationale.trim().is_empty() || !canonical(&decision.required_action_order)
            })
        {
            return Err(MultimodalReadinessError::InvalidOutput(
                "identity, metrics, surface order, decisions, action order, or digest fields are invalid"
                    .into(),
            ));
        }
        self.campaign
            .validate()
            .map_err(|error| MultimodalReadinessError::InvalidOutput(error.to_string()))?;
        if self.campaign_digest != self.campaign.digest
            || self.final_report_digest != self.campaign.final_report.digest
            || self.observation_count != self.campaign.observations.len()
            || self.comparable_observation_count
                != self.campaign.final_report.comparable_order.len()
            || self.simulation_only != self.campaign.simulation_only
        {
            return Err(MultimodalReadinessError::InvalidOutput(
                "campaign bindings or observation counts do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalReadinessError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalReadinessError::InvalidOutput(
                "multimodal readiness digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn coverage(required: usize, missing: usize) -> u16 {
    if required == 0 {
        1_000
    } else {
        (((required.saturating_sub(missing)) as u64 * 1_000) / required as u64) as u16
    }
}

fn actions_for_report(report: &MultimodalQcReport) -> Vec<String> {
    let mut actions = BTreeSet::new();
    if !report.missing_modality_order.is_empty() {
        actions.insert("acquire-missing-modalities".to_string());
    }
    if !report.missing_model_order.is_empty() {
        actions.insert("acquire-missing-model-systems".to_string());
    }
    if !report.excluded_order.is_empty() || !report.defect_order.is_empty() {
        actions.insert("repair-qc-defects-and-reingest".to_string());
    }
    if report.comparable_order.is_empty() {
        actions.insert("increase-comparable-observations".to_string());
    }
    actions.into_iter().collect()
}

fn evaluate_surface(
    surface: MultimodalResearchSurface,
    report: &MultimodalQcReport,
    request: &MultimodalReadinessRequest,
    coverage_milli: u16,
    quality_milli: u16,
    actions: &[String],
) -> MultimodalSurfaceDecision {
    let sufficient = report.comparable_order.len() >= request.min_comparable_observations
        && coverage_milli >= request.min_coverage_milli
        && quality_milli >= request.min_quality_milli;
    let complete = report.disposition == MultimodalDisposition::Qualified
        && sufficient
        && (!request.require_complete_report || report.excluded_order.is_empty());
    let multimodal = request.campaign.request.required_modalities.len() >= 2;
    let multi_model = request.campaign.request.required_model_systems.len() >= 2;
    let (readiness, rationale) = match surface {
        MultimodalResearchSurface::Analysis if complete => (
            MultimodalSurfaceReadiness::Admitted,
            "qualified comparable observations satisfy the analysis gate".into(),
        ),
        MultimodalResearchSurface::Analysis if sufficient => (
            MultimodalSurfaceReadiness::Conditional,
            "analysis may run with explicit QC limitations and omitted evidence".into(),
        ),
        MultimodalResearchSurface::Mechanism if complete && multimodal => (
            MultimodalSurfaceReadiness::Admitted,
            "qualified multimodal observations support mechanism exploration".into(),
        ),
        MultimodalResearchSurface::Mechanism if sufficient && multimodal => (
            MultimodalSurfaceReadiness::Conditional,
            "mechanism exploration is limited by partial multimodal QC".into(),
        ),
        MultimodalResearchSurface::ExperimentDesign if complete => (
            MultimodalSurfaceReadiness::Admitted,
            "qualified observations may inform a new preclinical design".into(),
        ),
        MultimodalResearchSurface::ExperimentDesign if sufficient => (
            MultimodalSurfaceReadiness::Conditional,
            "design may proceed only with explicit measurement and QC gaps".into(),
        ),
        MultimodalResearchSurface::Replication if complete && multi_model => (
            MultimodalSurfaceReadiness::Admitted,
            "qualified coverage spans the requested independent model systems".into(),
        ),
        MultimodalResearchSurface::Replication if sufficient => (
            MultimodalSurfaceReadiness::Conditional,
            "replication is allowed as a gap-closing activity, not as a qualification claim".into(),
        ),
        MultimodalResearchSurface::Publication if complete && report.uncertainty.is_empty() => (
            MultimodalSurfaceReadiness::Admitted,
            "complete QC and no unresolved report uncertainty remain".into(),
        ),
        MultimodalResearchSurface::Publication if sufficient => (
            MultimodalSurfaceReadiness::Conditional,
            "publication requires explicit limitations and unresolved QC disclosure".into(),
        ),
        _ if report.comparable_order.is_empty() => (
            MultimodalSurfaceReadiness::Unresolved,
            "no comparable observation supports downstream scientific work".into(),
        ),
        _ => (
            MultimodalSurfaceReadiness::Blocked,
            "coverage or quality thresholds are not satisfied".into(),
        ),
    };
    MultimodalSurfaceDecision {
        surface,
        readiness,
        rationale,
        required_action_order: if matches!(readiness, MultimodalSurfaceReadiness::Admitted) {
            Vec::new()
        } else {
            actions.to_vec()
        },
    }
}

fn overall_disposition(
    decisions: &[MultimodalSurfaceDecision],
    required: &BTreeSet<MultimodalResearchSurface>,
) -> MultimodalResearchReadinessDisposition {
    if decisions.is_empty() {
        return MultimodalResearchReadinessDisposition::Unresolved;
    }
    let required_decisions = decisions
        .iter()
        .filter(|decision| required.contains(&decision.surface))
        .collect::<Vec<_>>();
    if required_decisions.iter().any(|decision| {
        matches!(
            decision.readiness,
            MultimodalSurfaceReadiness::Blocked | MultimodalSurfaceReadiness::Unresolved
        )
    }) {
        return MultimodalResearchReadinessDisposition::Blocked;
    }
    if required_decisions
        .iter()
        .all(|decision| decision.readiness == MultimodalSurfaceReadiness::Admitted)
    {
        MultimodalResearchReadinessDisposition::Ready
    } else {
        MultimodalResearchReadinessDisposition::Conditional
    }
}

/// Execute a multimodal ingestion campaign and convert its final QC state into downstream
/// research admissions. The caller owns the executor; MCP exposes the same gate with a dry-run
/// adapter and therefore never treats synthetic observations as biological evidence.
pub fn execute_glioma_multimodal_readiness_gate<E: MultimodalIngestionCampaignExecutor>(
    request: &MultimodalReadinessRequest,
    executor: &mut E,
) -> Result<MultimodalResearchReadiness, MultimodalReadinessError> {
    if request.min_comparable_observations == 0
        || request.min_coverage_milli > 1_000
        || request.min_quality_milli > 1_000
        || request.required_surfaces.is_empty()
        || request.required_surfaces.len() > MAX_SURFACES
    {
        return Err(MultimodalReadinessError::InvalidRequest(
            "positive comparable floor, bounded coverage/quality thresholds, and required surfaces are required"
                .into(),
        ));
    }
    let campaign = execute_glioma_multimodal_ingestion_campaign(&request.campaign, executor)?;
    let report = &campaign.final_report;
    let modality_coverage = coverage(
        request.campaign.request.required_modalities.len(),
        report.missing_modality_order.len(),
    );
    let model_coverage = coverage(
        request.campaign.request.required_model_systems.len(),
        report.missing_model_order.len(),
    );
    let coverage_milli = modality_coverage.min(model_coverage);
    let quality_milli = if campaign.observations.is_empty() {
        0
    } else {
        ((report.comparable_order.len() as u64 * 1_000) / campaign.observations.len() as u64)
            .min(1_000) as u16
    };
    let surface_order = request
        .required_surfaces
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let actions = actions_for_report(report);
    let decisions = surface_order
        .iter()
        .copied()
        .map(|surface| {
            evaluate_surface(
                surface,
                report,
                request,
                coverage_milli,
                quality_milli,
                &actions,
            )
        })
        .collect::<Vec<_>>();
    let admitted_surface_order = decisions
        .iter()
        .filter(|decision| decision.readiness == MultimodalSurfaceReadiness::Admitted)
        .map(|decision| decision.surface)
        .collect::<Vec<_>>();
    let conditional_surface_order = decisions
        .iter()
        .filter(|decision| decision.readiness == MultimodalSurfaceReadiness::Conditional)
        .map(|decision| decision.surface)
        .collect::<Vec<_>>();
    let blocked_surface_order = decisions
        .iter()
        .filter(|decision| decision.readiness == MultimodalSurfaceReadiness::Blocked)
        .map(|decision| decision.surface)
        .collect::<Vec<_>>();
    let unresolved_surface_order = decisions
        .iter()
        .filter(|decision| decision.readiness == MultimodalSurfaceReadiness::Unresolved)
        .map(|decision| decision.surface)
        .collect::<Vec<_>>();
    let mut negative_evidence = report.negative_evidence.clone();
    if !blocked_surface_order.is_empty() {
        negative_evidence.push("downstream-surface-admission-blocked".into());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = report.uncertainty.clone();
    if coverage_milli < request.min_coverage_milli {
        uncertainty.push("coverage-below-readiness-floor".into());
    }
    if quality_milli < request.min_quality_milli {
        uncertainty.push("comparable-quality-below-readiness-floor".into());
    }
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = overall_disposition(&decisions, &request.required_surfaces);
    let mut output = MultimodalResearchReadiness {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: campaign.study_id.clone(),
        campaign_digest: campaign.digest.clone(),
        final_report_digest: report.digest.clone(),
        modality_coverage_milli: modality_coverage,
        model_coverage_milli: model_coverage,
        coverage_milli,
        quality_milli,
        comparable_observation_count: report.comparable_order.len(),
        observation_count: campaign.observations.len(),
        surface_order,
        decisions,
        admitted_surface_order,
        conditional_surface_order,
        blocked_surface_order,
        unresolved_surface_order,
        next_action_order: actions,
        negative_evidence,
        uncertainty,
        simulation_only: campaign.simulation_only,
        disposition,
        campaign,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-readiness"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalReadinessError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::campaign::DryRunMultimodalIngestionCampaignExecutor;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

    fn request() -> MultimodalReadinessRequest {
        MultimodalReadinessRequest {
            campaign: MultimodalIngestionCampaignRequest {
                request: crate::glioma::multimodal::MultimodalRequest {
                    study_id: "study-readiness".into(),
                    required_modalities: [GliomaModality::Genomics, GliomaModality::Imaging]
                        .into_iter()
                        .collect(),
                    required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
                    expected_coordinate_system: "sample-local".into(),
                    expected_unit_system: "normalized".into(),
                    max_missing_fraction_milli: 100,
                },
                observations: vec![crate::glioma::multimodal::MultimodalObservation {
                    observation_id: "obs-genomics".into(),
                    study_id: "study-readiness".into(),
                    sample_lineage: "sample-1".into(),
                    modality: GliomaModality::Genomics,
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: "batch-1".into(),
                    coordinate_system: "sample-local".into(),
                    unit_system: "normalized".into(),
                    missing_fraction_milli: 10,
                    feature_count: 100,
                    artifact: LocalArtifactRef {
                        artifact_id: "obs-artifact".into(),
                        content_hash: ContentHash::of_bytes(b"obs-artifact"),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                }],
                max_actions_per_round: 4,
                budget_units: 8,
                cost_per_action_units: 2,
                max_rounds: 4,
                max_retries: 2,
                stop_on_qualified: true,
            },
            min_comparable_observations: 1,
            min_coverage_milli: 1_000,
            min_quality_milli: 800,
            require_complete_report: true,
            required_surfaces: [
                MultimodalResearchSurface::Analysis,
                MultimodalResearchSurface::Mechanism,
                MultimodalResearchSurface::ExperimentDesign,
                MultimodalResearchSurface::Replication,
            ]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn readiness_gate_executes_qc_campaign_and_admits_only_supported_surfaces() {
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let output = execute_glioma_multimodal_readiness_gate(&request(), &mut executor).unwrap();
        assert!(output.simulation_only);
        assert_eq!(output.coverage_milli, 1_000);
        assert!(output
            .admitted_surface_order
            .contains(&MultimodalResearchSurface::Analysis));
        assert!(output
            .conditional_surface_order
            .contains(&MultimodalResearchSurface::Replication));
        assert!(output.next_action_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn readiness_gate_keeps_missing_modality_and_downstream_blocks_explicit() {
        let mut request = request();
        request.min_coverage_milli = 1_000;
        request.campaign.budget_units = 0;
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let error = execute_glioma_multimodal_readiness_gate(&request, &mut executor).unwrap_err();
        assert!(matches!(error, MultimodalReadinessError::Campaign(_)));
    }
}
