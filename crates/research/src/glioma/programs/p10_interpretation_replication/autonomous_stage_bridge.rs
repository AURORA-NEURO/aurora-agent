//! P07 autonomous-engine bridges for P10 interpretation and replication stages.
//!
//! These workers make the last scientific stages of the autonomous glioma graph executable
//! through the existing P10 operating-cycle and replication-campaign algorithms. They are
//! composition seams, not new catalog slots: a caller supplies the typed P10 request and either
//! a governed local executor or the deterministic dry-run executor. Neither worker promotes a
//! statistical result into a clinical decision or silently treats a dry-run artifact as biology.

use super::campaign::{
    execute_glioma_replication_campaign, DryRunGliomaReplicationCampaignExecutor,
    GliomaReplicationCampaign, GliomaReplicationCampaignDisposition,
    GliomaReplicationCampaignExecutor, GliomaReplicationCampaignRequest,
};
use super::operating_cycle::{
    execute_glioma_interpretation_operating_cycle, GliomaInterpretationOperatingCycle,
    GliomaInterpretationOperatingCycleError, GliomaInterpretationOperatingCycleRequest,
    InterpretationOperatingCycleDisposition,
};
use crate::glioma_engine::{
    GliomaStage, GliomaStageDisposition, GliomaStageExecutor, GliomaStageFailure, GliomaStageInput,
    GliomaStageKind, GliomaStageOutput,
};
use bioprism_foundation::TypedResearchArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const INTERPRETATION_PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P10-F24";
pub const INTERPRETATION_OUTPUT_SCHEMA: &str = "GliomaInterpretationAutonomousStageBridge1@1";
pub const REPLICATION_PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P10-F18";
pub const REPLICATION_OUTPUT_SCHEMA: &str = "GliomaReplicationAutonomousStageBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaInterpretationStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub operating_cycle: GliomaInterpretationOperatingCycle,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub campaign: GliomaReplicationCampaign,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaInterpretationStageBridgeError {
    #[error("interpretation stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("interpretation operating cycle failed: {0}")]
    OperatingCycle(#[from] GliomaInterpretationOperatingCycleError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaReplicationStageBridgeError {
    #[error("replication stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication campaign failed: {0}")]
    Campaign(String),
}

pub struct GliomaInterpretationStageWorker {
    request: GliomaInterpretationOperatingCycleRequest,
    simulation_only: bool,
}

impl GliomaInterpretationStageWorker {
    pub fn new(
        request: GliomaInterpretationOperatingCycleRequest,
        simulation_only: bool,
    ) -> Result<Self, GliomaInterpretationStageBridgeError> {
        if request.synthesis.objective.trim().is_empty()
            || request.synthesis.hypothesis.trim().is_empty()
            || request.budget_units == 0
            || request.max_actions == 0
        {
            return Err(GliomaInterpretationStageBridgeError::InvalidRequest(
                "interpretation objective, hypothesis, budget, and action bound are required"
                    .into(),
            ));
        }
        Ok(Self {
            request,
            simulation_only,
        })
    }

    pub fn request(&self) -> &GliomaInterpretationOperatingCycleRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::StatisticalInterpretation
            || stage.output_schema != GliomaStageKind::StatisticalInterpretation.output_schema()
        {
            return Err(GliomaStageFailure {
                reason:
                    "interpretation bridge received a non-statistical-interpretation stage contract"
                        .into(),
                retryable: false,
            });
        }
        let cycle =
            execute_glioma_interpretation_operating_cycle(&self.request).map_err(|error| {
                GliomaStageFailure {
                    reason: error.to_string(),
                    retryable: false,
                }
            })?;
        let receipt = GliomaInterpretationStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: self.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: cycle.uncertainty.clone(),
            next_required_gate: "replication-robustness-adjudication".into(),
            operating_cycle: cycle.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("interpretation-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("interpretation bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("interpretation bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match cycle.disposition {
            InterpretationOperatingCycleDisposition::Ready => GliomaStageDisposition::Completed,
            InterpretationOperatingCycleDisposition::Negative => GliomaStageDisposition::Negative,
            InterpretationOperatingCycleDisposition::Partial => GliomaStageDisposition::Partial,
            InterpretationOperatingCycleDisposition::Blocked
            | InterpretationOperatingCycleDisposition::Unresolved => {
                GliomaStageDisposition::Blocked
            }
        };
        let mut uncertainty = cycle.uncertainty.clone();
        uncertainty.push("interpretation-requires-independent-replication-and-release-gate".into());
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = cycle.negative_evidence.clone();
        negative_evidence
            .push("interpretation-output-does-not-authorize-instrument-or-clinical-action".into());
        negative_evidence.sort();
        negative_evidence.dedup();
        Ok(GliomaStageOutput {
            artifact,
            disposition,
            uncertainty,
            negative_evidence,
        })
    }
}

impl GliomaStageExecutor for GliomaInterpretationStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

struct DynGliomaReplicationExecutor {
    inner: Box<dyn GliomaReplicationCampaignExecutor>,
}

impl GliomaReplicationCampaignExecutor for DynGliomaReplicationExecutor {
    fn execute_action(
        &mut self,
        action: &super::campaign::GliomaReplicationAction,
        round: u16,
    ) -> Result<
        super::campaign::GliomaReplicationCampaignObservation,
        super::campaign::GliomaReplicationExecutionFailure,
    > {
        self.inner.execute_action(action, round)
    }
}

pub struct GliomaReplicationStageWorker {
    request: GliomaReplicationCampaignRequest,
    executor: DynGliomaReplicationExecutor,
    simulation_only: bool,
}

impl GliomaReplicationStageWorker {
    pub fn new(
        request: GliomaReplicationCampaignRequest,
        executor: Box<dyn GliomaReplicationCampaignExecutor>,
        simulation_only: bool,
    ) -> Result<Self, GliomaReplicationStageBridgeError> {
        if request.objective.trim().is_empty()
            || request.target_signature.is_empty()
            || request.budget_units == 0
            || request.max_rounds == 0
        {
            return Err(GliomaReplicationStageBridgeError::InvalidRequest(
                "replication objective, target signature, budget, and round bound are required"
                    .into(),
            ));
        }
        Ok(Self {
            request,
            executor: DynGliomaReplicationExecutor { inner: executor },
            simulation_only,
        })
    }

    pub fn request(&self) -> &GliomaReplicationCampaignRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::ReplicationRobustness
            || stage.output_schema != GliomaStageKind::ReplicationRobustness.output_schema()
        {
            return Err(GliomaStageFailure {
                reason: "replication bridge received a non-replication stage contract".into(),
                retryable: false,
            });
        }
        let campaign = execute_glioma_replication_campaign(&self.request, &mut self.executor)
            .map_err(|error| GliomaStageFailure {
                reason: error.to_string(),
                retryable: false,
            })?;
        let receipt = GliomaReplicationStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: self.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: campaign.uncertainty.clone(),
            next_required_gate: "research-object-release-and-federated-benchmarking".into(),
            campaign: campaign.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("replication-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("replication bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("replication bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match campaign.disposition {
            GliomaReplicationCampaignDisposition::Qualified => GliomaStageDisposition::Completed,
            GliomaReplicationCampaignDisposition::Negative => GliomaStageDisposition::Negative,
            GliomaReplicationCampaignDisposition::Partial => GliomaStageDisposition::Partial,
            GliomaReplicationCampaignDisposition::Unresolved
            | GliomaReplicationCampaignDisposition::Failed
            | GliomaReplicationCampaignDisposition::Blocked => GliomaStageDisposition::Blocked,
        };
        let mut uncertainty = campaign.uncertainty.clone();
        uncertainty.push(
            "replication-result-requires-research-object-release-and-federated-review".into(),
        );
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = campaign.negative_evidence.clone();
        negative_evidence
            .push("replication-assessment-does-not-authorize-clinical-decision".into());
        negative_evidence.sort();
        negative_evidence.dedup();
        Ok(GliomaStageOutput {
            artifact,
            disposition,
            uncertainty,
            negative_evidence,
        })
    }
}

impl GliomaStageExecutor for GliomaReplicationStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

pub fn dry_run_glioma_interpretation_stage_worker(
    request: GliomaInterpretationOperatingCycleRequest,
) -> Result<GliomaInterpretationStageWorker, GliomaInterpretationStageBridgeError> {
    GliomaInterpretationStageWorker::new(request, true)
}

pub fn dry_run_glioma_replication_stage_worker(
    request: GliomaReplicationCampaignRequest,
) -> Result<GliomaReplicationStageWorker, GliomaReplicationStageBridgeError> {
    GliomaReplicationStageWorker::new(
        request,
        Box::new(DryRunGliomaReplicationCampaignExecutor::default()),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::synthesis::{
        InterpretationEvidence, InterpretationEvidenceDirection, InterpretationEvidenceFamily,
        InterpretationSynthesisRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;
    use std::collections::BTreeSet;

    fn input(stage: &GliomaStage) -> GliomaStageInput {
        GliomaStageInput {
            research_id: "research-p10-bridge".into(),
            study_id: "study-p10-bridge".into(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: Vec::new(),
            source_artifacts: Vec::new(),
            replay_identity: ContentHash::of_bytes(b"p10-bridge-stage-replay"),
            attempt: 1,
        }
    }

    fn interpretation_request() -> GliomaInterpretationOperatingCycleRequest {
        let evidence = [
            ("causal", InterpretationEvidenceFamily::CausalContrast),
            ("replication", InterpretationEvidenceFamily::Replication),
            ("sensitivity", InterpretationEvidenceFamily::Sensitivity),
        ]
        .into_iter()
        .map(|(id, family)| InterpretationEvidence {
            evidence_id: id.into(),
            family,
            independent_group: id.into(),
            model_system: GliomaModelSystem::Organoid,
            direction: InterpretationEvidenceDirection::Positive,
            effect_milli: 400,
            uncertainty_milli: 40,
            quality_milli: 900,
            sample_count: 8,
            artifact: LocalArtifactRef {
                artifact_id: id.into(),
                content_hash: ContentHash::of_bytes(b"p10-bridge-evidence"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            negative_evidence: Vec::new(),
        })
        .collect();
        GliomaInterpretationOperatingCycleRequest {
            synthesis: InterpretationSynthesisRequest {
                objective: "decide whether organoid invasion is reproducibly supported".into(),
                hypothesis: "integrated invasion program is activated".into(),
                model_system: GliomaModelSystem::Organoid,
                min_evidence: 2,
                min_independent_groups: 2,
                min_families: 2,
                min_quality_milli: 700,
                effect_threshold_milli: 100,
                max_disagreement_milli: 700,
                max_leave_one_out_shift_milli: 700,
                require_replication_family: true,
                replay_identity: ContentHash::of_bytes(b"p10-bridge-synthesis-replay"),
                evidence,
            },
            completed_actions: BTreeSet::new(),
            budget_units: 80,
            max_actions: 3,
            approval_granted: true,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: crate::glioma_engine::GliomaSelectionWeights::default(),
        }
    }

    fn replication_request() -> GliomaReplicationCampaignRequest {
        GliomaReplicationCampaignRequest {
            objective: "replicate a preclinical glioma invasion effect".into(),
            model_system: GliomaModelSystem::Organoid,
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![1, 2],
            min_sites: 3,
            min_replicates_per_site: 2,
            min_studies: 3,
            min_replicates_per_study: 2,
            effect_threshold_milli: 10,
            max_heterogeneity_milli: 500,
            max_i2_milli: 500,
            min_signal_to_noise_milli: 10,
            max_leave_one_out_shift_milli: 1_000,
            min_quality_milli: 500,
            distance_scale_milli: 1_000,
            max_transport_gap_milli: 500,
            max_transport_heterogeneity_milli: 500,
            budget_units: 16,
            max_rounds: 2,
            max_actions_per_round: 1,
            max_retries: 1,
            initial_studies: Vec::new(),
            initial_transport_studies: Vec::new(),
            replay_identity: ContentHash::of_bytes(b"p10-bridge-replication-replay"),
        }
    }

    #[test]
    fn interpretation_and_replication_stage_workers_execute_and_preserve_boundaries() {
        let interpretation_stage = GliomaStage {
            stage_id: GliomaStageKind::StatisticalInterpretation.stage_id().into(),
            kind: GliomaStageKind::StatisticalInterpretation,
            output_schema: GliomaStageKind::StatisticalInterpretation
                .output_schema()
                .into(),
            depends_on: vec![GliomaStageKind::ComputationalExecution.stage_id().into()],
            input_schemas: vec![GliomaStageKind::ComputationalExecution
                .output_schema()
                .into()],
            required: true,
            readiness: crate::glioma_engine::StageReadiness::Ready,
            autonomy_tier: bioprism_foundation::AutonomyTier::A1,
            effects: std::collections::BTreeSet::new(),
            budget_units: 20,
        };
        let mut interpretation =
            dry_run_glioma_interpretation_stage_worker(interpretation_request()).unwrap();
        let interpretation_output = interpretation
            .execute(&interpretation_stage, &input(&interpretation_stage))
            .unwrap();
        interpretation_output.artifact.validate_metadata().unwrap();
        assert_eq!(
            interpretation_output.artifact.content_type,
            GliomaStageKind::StatisticalInterpretation.output_schema()
        );
        assert!(interpretation_output
            .uncertainty
            .iter()
            .any(|value| value.contains("independent-replication")));
        assert!(interpretation_output
            .negative_evidence
            .iter()
            .any(|value| value.contains("does-not-authorize")));

        let replication_stage = GliomaStage {
            stage_id: GliomaStageKind::ReplicationRobustness.stage_id().into(),
            kind: GliomaStageKind::ReplicationRobustness,
            output_schema: GliomaStageKind::ReplicationRobustness
                .output_schema()
                .into(),
            depends_on: vec![GliomaStageKind::StatisticalInterpretation.stage_id().into()],
            input_schemas: vec![GliomaStageKind::StatisticalInterpretation
                .output_schema()
                .into()],
            required: true,
            readiness: crate::glioma_engine::StageReadiness::Ready,
            autonomy_tier: bioprism_foundation::AutonomyTier::A1,
            effects: std::collections::BTreeSet::new(),
            budget_units: 16,
        };
        let mut replication =
            dry_run_glioma_replication_stage_worker(replication_request()).unwrap();
        let replication_output = replication
            .execute(&replication_stage, &input(&replication_stage))
            .unwrap();
        replication_output.artifact.validate_metadata().unwrap();
        assert_eq!(
            replication_output.artifact.content_type,
            GliomaStageKind::ReplicationRobustness.output_schema()
        );
        assert!(replication_output
            .negative_evidence
            .iter()
            .any(|value| value.contains("does-not-authorize-clinical")));
    }
}
