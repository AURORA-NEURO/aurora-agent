//! Multimodal ingestion-to-readiness operating cycle for preclinical glioma research.
//!
//! P03 must do more than ingest files: it must decide which downstream scientific surfaces can
//! safely consume the current metadata. This feature executes the QC campaign, evaluates surface
//! admission, and returns a concrete next handoff while preserving missingness, defects, and
//! conditional readiness. Raw payloads remain in institution-local adapters.

use super::campaign::MultimodalIngestionCampaignExecutor;
use super::readiness_gate::{
    execute_glioma_multimodal_readiness_gate, MultimodalReadinessError, MultimodalReadinessRequest,
    MultimodalResearchReadiness, MultimodalResearchReadinessDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMultimodalOperatingCycleRequest {
    pub readiness: MultimodalReadinessRequest,
    pub execution_mode: MultimodalExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMultimodalOperatingCycleDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMultimodalOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub phase_order: Vec<String>,
    pub readiness: MultimodalResearchReadiness,
    pub simulation_only: bool,
    pub execution_mode: MultimodalExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaMultimodalOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaMultimodalOperatingCycleError {
    #[error("multimodal operating-cycle readiness failed: {0}")]
    Readiness(#[from] MultimodalReadinessError),
    #[error("multimodal operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaMultimodalOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "phase_order": output.phase_order,
        "readiness": output.readiness,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn disposition(
    readiness: MultimodalResearchReadinessDisposition,
) -> GliomaMultimodalOperatingCycleDisposition {
    match readiness {
        MultimodalResearchReadinessDisposition::Ready => {
            GliomaMultimodalOperatingCycleDisposition::Ready
        }
        MultimodalResearchReadinessDisposition::Conditional => {
            GliomaMultimodalOperatingCycleDisposition::Conditional
        }
        MultimodalResearchReadinessDisposition::Blocked => {
            GliomaMultimodalOperatingCycleDisposition::Blocked
        }
        MultimodalResearchReadinessDisposition::Unresolved => {
            GliomaMultimodalOperatingCycleDisposition::Unresolved
        }
    }
}

impl GliomaMultimodalOperatingCycle {
    pub fn validate(&self) -> Result<(), GliomaMultimodalOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.phase_order
                != [
                    "ingestion_qc".to_string(),
                    "surface_readiness".to_string(),
                    "operator_handoff".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || self.readiness.study_id != self.study_id
            || self.simulation_only
                != matches!(
                    self.execution_mode,
                    MultimodalExecutionMode::LocalSimulation
                )
            || self.readiness.simulation_only != self.simulation_only
        {
            return Err(GliomaMultimodalOperatingCycleError::InvalidOutput(
                "identity, phases, readiness binding, evidence ordering, or execution mode is invalid".into(),
            ));
        }
        self.readiness.validate().map_err(|error| {
            GliomaMultimodalOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaMultimodalOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaMultimodalOperatingCycleError::InvalidOutput(
                "multimodal operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run metadata ingestion/QC and convert the resulting report into downstream surface admission.
pub fn execute_glioma_multimodal_operating_cycle<E: MultimodalIngestionCampaignExecutor>(
    request: &GliomaMultimodalOperatingCycleRequest,
    executor: &mut E,
) -> Result<GliomaMultimodalOperatingCycle, GliomaMultimodalOperatingCycleError> {
    let readiness = execute_glioma_multimodal_readiness_gate(&request.readiness, executor)?;
    let cycle_disposition = disposition(readiness.disposition);
    let mut negative_evidence = readiness.negative_evidence.clone();
    let mut uncertainty = readiness.uncertainty.clone();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = if let Some(action) = readiness.next_action_order.first() {
        format!("execute the highest-priority multimodal QC remediation: {action}")
    } else {
        match cycle_disposition {
            GliomaMultimodalOperatingCycleDisposition::Ready => {
                "route admitted surfaces to mechanism, computation, and interpretation workflows while preserving the QC digest".into()
            }
            GliomaMultimodalOperatingCycleDisposition::Conditional => {
                "use only admitted surfaces and resolve conditional modality/model coverage before promotion".into()
            }
            GliomaMultimodalOperatingCycleDisposition::Blocked => {
                "resolve missingness, quality defects, or blocked surface prerequisites before downstream execution".into()
            }
            GliomaMultimodalOperatingCycleDisposition::Unresolved => {
                "obtain comparable local observations before treating the study as research-ready".into()
            }
        }
    };
    let mut output = GliomaMultimodalOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: readiness.study_id.clone(),
        phase_order: vec![
            "ingestion_qc".into(),
            "surface_readiness".into(),
            "operator_handoff".into(),
        ],
        simulation_only: matches!(
            request.execution_mode,
            MultimodalExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        readiness,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition: cycle_disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaMultimodalOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_multimodal_operating_cycle_dry_run(
    request: &GliomaMultimodalOperatingCycleRequest,
) -> Result<GliomaMultimodalOperatingCycle, GliomaMultimodalOperatingCycleError> {
    let mut executor = super::campaign::DryRunMultimodalIngestionCampaignExecutor;
    execute_glioma_multimodal_operating_cycle(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::multimodal::{MultimodalObservation, MultimodalRequest};
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use std::collections::BTreeSet;

    fn request() -> GliomaMultimodalOperatingCycleRequest {
        GliomaMultimodalOperatingCycleRequest {
            readiness: MultimodalReadinessRequest {
                campaign: super::super::campaign::MultimodalIngestionCampaignRequest {
                    request: MultimodalRequest {
                        study_id: "operating-study".into(),
                        required_modalities: BTreeSet::from([
                            GliomaModality::Genomics,
                            GliomaModality::Imaging,
                        ]),
                        required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                        expected_coordinate_system: "sample-local".into(),
                        expected_unit_system: "normalized".into(),
                        max_missing_fraction_milli: 100,
                    },
                    observations: vec![MultimodalObservation {
                        observation_id: "genomics-observation".into(),
                        study_id: "operating-study".into(),
                        sample_lineage: "sample-1".into(),
                        modality: GliomaModality::Genomics,
                        model_system: GliomaModelSystem::Organoid,
                        batch_id: "batch-1".into(),
                        coordinate_system: "sample-local".into(),
                        unit_system: "normalized".into(),
                        missing_fraction_milli: 10,
                        feature_count: 100,
                        artifact: LocalArtifactRef {
                            artifact_id: "genomics-artifact".into(),
                            content_hash: ContentHash::of_bytes(b"genomics-artifact"),
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
                required_surfaces: BTreeSet::from([
                    super::super::readiness_gate::MultimodalResearchSurface::Analysis,
                    super::super::readiness_gate::MultimodalResearchSurface::Mechanism,
                ]),
            },
            execution_mode: MultimodalExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn operating_cycle_replays_qc_and_surface_admission() {
        let first = execute_glioma_multimodal_operating_cycle_dry_run(&request()).unwrap();
        let second = execute_glioma_multimodal_operating_cycle_dry_run(&request()).unwrap();
        assert_eq!(first, second);
        assert!(first.simulation_only);
        assert_eq!(first.phase_order[0], "ingestion_qc");
        assert!(first
            .readiness
            .admitted_surface_order
            .contains(&super::super::readiness_gate::MultimodalResearchSurface::Analysis));
        first.validate().unwrap();
    }
}
