//! P07 autonomous-engine bridge for the governed P09 computation operating cycle.
//!
//! This adapter turns the typed `computational-execution` stage into a real call to P09's
//! workflow compiler, resource gate, replay-keyed campaign, and local executor seam. It keeps
//! computation separate from interpretation: a completed local run is a typed stage artifact,
//! not a biological conclusion, and partial/negative/blocked outcomes remain first-class.

use super::campaign::StaticGliomaComputationPlanner;
use super::execution::{
    ComputationExecutionFailure, ComputationTask, ComputationTaskResult, GliomaComputationExecutor,
};
use super::operating_cycle::{
    execute_glioma_computation_operating_cycle, GliomaComputationOperatingCycle,
    GliomaComputationOperatingCycleDisposition, GliomaComputationOperatingCycleError,
    GliomaComputationOperatingCycleRequest,
};
use crate::glioma_engine::{
    GliomaStage, GliomaStageDisposition, GliomaStageExecutor, GliomaStageFailure, GliomaStageInput,
    GliomaStageKind, GliomaStageOutput,
};
use bioprism_foundation::TypedResearchArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Composition of the existing P09-F24 operating-cycle feature and P07's autonomous worker
/// seam; no additional stable catalog slot is consumed.
pub const PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P09-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationAutonomousStageBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub operating_cycle: GliomaComputationOperatingCycle,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaComputationStageBridgeError {
    #[error("computation stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation operating cycle failed: {0}")]
    OperatingCycle(#[from] GliomaComputationOperatingCycleError),
}

/// Box the P09 executor so one route registry can hold workers for multiple programs.
struct DynGliomaComputationExecutor {
    inner: Box<dyn GliomaComputationExecutor>,
}

impl GliomaComputationExecutor for DynGliomaComputationExecutor {
    fn execute_task(
        &mut self,
        task: &ComputationTask,
        upstream: &[ComputationTaskResult],
        attempt: u8,
    ) -> Result<ComputationTaskResult, ComputationExecutionFailure> {
        self.inner.execute_task(task, upstream, attempt)
    }
}

/// Institution-local P09 worker. Production hosts inject a governed executor; MCP and tests use
/// the deterministic local simulation helper below.
pub struct GliomaComputationStageWorker {
    request: GliomaComputationOperatingCycleRequest,
    executor: DynGliomaComputationExecutor,
}

impl GliomaComputationStageWorker {
    pub fn new(
        request: GliomaComputationOperatingCycleRequest,
        executor: Box<dyn GliomaComputationExecutor>,
    ) -> Result<Self, GliomaComputationStageBridgeError> {
        if request.workflow.objective.trim().is_empty()
            || request.workflow.study_id.trim().is_empty()
            || request.workflow.input_artifact_ids.is_empty()
        {
            return Err(GliomaComputationStageBridgeError::InvalidRequest(
                "computation workflow objective, study identity, and local inputs are required"
                    .into(),
            ));
        }
        Ok(Self {
            request,
            executor: DynGliomaComputationExecutor { inner: executor },
        })
    }

    pub fn request(&self) -> &GliomaComputationOperatingCycleRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::ComputationalExecution
            || stage.output_schema != GliomaStageKind::ComputationalExecution.output_schema()
        {
            return Err(GliomaStageFailure {
                reason: "computation bridge received a non-computation stage contract".into(),
                retryable: false,
            });
        }
        let mut planner = StaticGliomaComputationPlanner;
        let cycle = execute_glioma_computation_operating_cycle(
            &self.request,
            &mut planner,
            &mut self.executor,
        )
        .map_err(|error| GliomaStageFailure {
            reason: error.to_string(),
            retryable: false,
        })?;
        let receipt = GliomaComputationStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: cycle.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: cycle.uncertainty.clone(),
            next_required_gate: "statistical-interpretation-and-reproducibility-adjudication"
                .into(),
            operating_cycle: cycle.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("computation-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("computation bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("computation bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match cycle.disposition {
            GliomaComputationOperatingCycleDisposition::Executed => {
                GliomaStageDisposition::Completed
            }
            GliomaComputationOperatingCycleDisposition::Negative => {
                GliomaStageDisposition::Negative
            }
            GliomaComputationOperatingCycleDisposition::Partial => GliomaStageDisposition::Partial,
            GliomaComputationOperatingCycleDisposition::Blocked
            | GliomaComputationOperatingCycleDisposition::Failed
            | GliomaComputationOperatingCycleDisposition::Unresolved => {
                GliomaStageDisposition::Blocked
            }
        };
        let mut uncertainty = cycle.uncertainty.clone();
        uncertainty.push("computation-output-is-not-statistical-interpretation".into());
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = cycle.negative_evidence.clone();
        negative_evidence.push(
            "computation-run-requires-statistical-interpretation-and-reproducibility-adjudication"
                .into(),
        );
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

impl GliomaStageExecutor for GliomaComputationStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

/// Construct a worker backed by P09's deterministic local planner and computation executor.
pub fn dry_run_glioma_computation_stage_worker(
    request: GliomaComputationOperatingCycleRequest,
) -> Result<GliomaComputationStageWorker, GliomaComputationStageBridgeError> {
    GliomaComputationStageWorker::new(
        request,
        Box::new(super::execution::DryRunGliomaComputationExecutor),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::{
        ComputationExecutionMode, ComputationOperation, GliomaComputationWorkflowRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn request() -> GliomaComputationOperatingCycleRequest {
        GliomaComputationOperatingCycleRequest {
            workflow: GliomaComputationWorkflowRequest {
                objective: "profile invasive organoid state across modalities".into(),
                study_id: "study-glioma-bridge".into(),
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
                replay_identity: ContentHash::of_bytes(b"bridge-workflow-replay"),
            },
            require_within_resources: true,
            execution_mode: ComputationExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn computation_stage_bridge_executes_p09_and_preserves_interpretation_boundary() {
        let mut worker = dry_run_glioma_computation_stage_worker(request()).unwrap();
        let stage = GliomaStage {
            stage_id: "computational-execution".into(),
            kind: GliomaStageKind::ComputationalExecution,
            output_schema: GliomaStageKind::ComputationalExecution
                .output_schema()
                .into(),
            depends_on: Vec::new(),
            input_schemas: Vec::new(),
            required: true,
            readiness: crate::glioma_engine::StageReadiness::Ready,
            autonomy_tier: bioprism_foundation::AutonomyTier::A1,
            effects: std::collections::BTreeSet::new(),
            budget_units: 24,
        };
        let input = GliomaStageInput {
            research_id: "research-bridge".into(),
            study_id: "study-glioma-bridge".into(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: Vec::new(),
            source_artifacts: vec![LocalArtifactRef {
                artifact_id: "source-rna".into(),
                content_hash: ContentHash::of_bytes(b"source-rna"),
                content_type: "application/vnd.aurora.glioma.rna+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            replay_identity: ContentHash::of_bytes(b"stage-replay"),
            attempt: 1,
        };
        let output = worker.execute(&stage, &input).unwrap();
        output.artifact.validate_metadata().unwrap();
        assert_eq!(output.disposition, GliomaStageDisposition::Completed);
        assert!(output
            .uncertainty
            .contains(&"computation-output-is-not-statistical-interpretation".into()));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("requires-statistical-interpretation")));
    }
}
