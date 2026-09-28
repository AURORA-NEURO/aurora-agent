//! P07 autonomous-engine bridge for the governed P11 research-object release cycle.
//!
//! The release stage is intentionally a gate, not a publishing side effect.  It replays the
//! exact local manifest, evaluates the accountable release predicates, and emits a typed
//! research-object candidate.  Signing, uploading, and federation remain separate approved
//! operations owned by the institution or consortium steward.

use super::operating_cycle::{
    execute_glioma_release_operating_cycle, GliomaReleaseOperatingCycle,
    GliomaReleaseOperatingCycleDisposition, GliomaReleaseOperatingCycleError,
    GliomaReleaseOperatingCycleRequest,
};
use super::replay::{
    DryRunReplayCampaignExecutor, ReplayCampaignExecutor, ReplayExecutionFailure,
    ReplayObservation, ReplayTask,
};
use crate::glioma_engine::{
    GliomaStage, GliomaStageDisposition, GliomaStageExecutor, GliomaStageFailure, GliomaStageInput,
    GliomaStageKind, GliomaStageOutput,
};
use bioprism_foundation::TypedResearchArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P11-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseAutonomousStageBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReleaseStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub operating_cycle: GliomaReleaseOperatingCycle,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaReleaseStageBridgeError {
    #[error("release stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release operating cycle failed: {0}")]
    OperatingCycle(#[from] GliomaReleaseOperatingCycleError),
}

struct DynReplayCampaignExecutor {
    inner: Box<dyn ReplayCampaignExecutor>,
}

impl ReplayCampaignExecutor for DynReplayCampaignExecutor {
    fn replay_task(
        &mut self,
        task: &ReplayTask,
        request: &super::replay::ReplayCampaignRequest,
        attempt: u8,
    ) -> Result<ReplayObservation, ReplayExecutionFailure> {
        self.inner.replay_task(task, request, attempt)
    }
}

/// Institution-local P11 worker.  A production host injects a replay executor; the public dry-run
/// constructor below is deterministic and never publishes or moves an artifact.
pub struct GliomaReleaseStageWorker {
    request: GliomaReleaseOperatingCycleRequest,
    executor: DynReplayCampaignExecutor,
}

impl GliomaReleaseStageWorker {
    pub fn new(
        request: GliomaReleaseOperatingCycleRequest,
        executor: Box<dyn ReplayCampaignExecutor>,
    ) -> Result<Self, GliomaReleaseStageBridgeError> {
        if request.replay.release.research_id.trim().is_empty()
            || request.replay.release.study_id.trim().is_empty()
            || request.replay.tasks.is_empty()
            || request.replay.budget_units == 0
            || request.replay.max_rounds == 0
        {
            return Err(GliomaReleaseStageBridgeError::InvalidRequest(
                "release identity, replay tasks, budget, and round bound are required".into(),
            ));
        }
        Ok(Self {
            request,
            executor: DynReplayCampaignExecutor { inner: executor },
        })
    }

    pub fn request(&self) -> &GliomaReleaseOperatingCycleRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::ResearchObjectRelease
            || stage.output_schema != GliomaStageKind::ResearchObjectRelease.output_schema()
        {
            return Err(GliomaStageFailure {
                reason: "release bridge received a non-research-object-release stage contract"
                    .into(),
                retryable: false,
            });
        }
        let cycle = execute_glioma_release_operating_cycle(&self.request, &mut self.executor)
            .map_err(|error| GliomaStageFailure {
                reason: error.to_string(),
                retryable: false,
            })?;
        let receipt = GliomaReleaseStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: cycle.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: cycle.uncertainty.clone(),
            next_required_gate: "accountable-signature-and-federated-benchmark-governance".into(),
            operating_cycle: cycle.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("release-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("release bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("release bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match cycle.disposition {
            GliomaReleaseOperatingCycleDisposition::Publishable => {
                GliomaStageDisposition::Completed
            }
            GliomaReleaseOperatingCycleDisposition::NonReproducible => {
                GliomaStageDisposition::Negative
            }
            GliomaReleaseOperatingCycleDisposition::Hold
            | GliomaReleaseOperatingCycleDisposition::Unresolved
            | GliomaReleaseOperatingCycleDisposition::Blocked => GliomaStageDisposition::Blocked,
        };
        let mut uncertainty = cycle.uncertainty.clone();
        uncertainty.push(
            "release-candidate-requires-accountable-signature-and-federated-governance-review"
                .into(),
        );
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = cycle.negative_evidence.clone();
        negative_evidence.push("release-stage-does-not-upload-or-sign-raw-data".into());
        negative_evidence.push("release-candidate-does-not-promote-biological-evidence".into());
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

impl GliomaStageExecutor for GliomaReleaseStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

pub fn dry_run_glioma_release_stage_worker(
    request: GliomaReleaseOperatingCycleRequest,
) -> Result<GliomaReleaseStageWorker, GliomaReleaseStageBridgeError> {
    GliomaReleaseStageWorker::new(request, Box::new(DryRunReplayCampaignExecutor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::{
        ReleaseExecutionMode, ReleaseGateRequest, ReleaseReviewAttestation, ReleaseReviewDecision,
        ReplayCampaignRequest, ReplayTask,
    };
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::{GliomaStage, LocalArtifactRef, StageReadiness};
    use bioprism_foundation::AutonomyTier;
    use bioprism_ids::ContentHash;

    fn request() -> GliomaReleaseOperatingCycleRequest {
        let artifact_hash = ContentHash::of_bytes(b"release-artifact");
        GliomaReleaseOperatingCycleRequest {
            replay: ReplayCampaignRequest {
                release: ResearchObjectRequest {
                    research_id: "bridge-release-research".into(),
                    study_id: "bridge-release-study".into(),
                    objective: "release a reproducible preclinical glioma object".into(),
                    plan_digest: ContentHash::of_bytes(b"plan"),
                    execution_digest: ContentHash::of_bytes(b"execution"),
                    replay_identity: ContentHash::of_bytes(b"replay"),
                    program_order: vec!["p09-computation".into()],
                    artifacts: vec![LocalArtifactRef {
                        artifact_id: "artifact-main".into(),
                        content_hash: artifact_hash.clone(),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    }],
                    negative_evidence: vec!["synthetic-fixture-only".into()],
                    limitations: vec!["dry-run".into()],
                    raw_data_local: true,
                    aggregate_only: true,
                },
                tasks: vec![ReplayTask {
                    task_id: "task-main".into(),
                    program_id: "p09-computation".into(),
                    artifact_id: "artifact-main".into(),
                    expected_content_hash: artifact_hash,
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: Vec::new(),
                }],
                budget_units: 4,
                max_rounds: 2,
                max_retries: 1,
                min_coverage_milli: 1_000,
                require_exact_hash: true,
            },
            gate: ReleaseGateRequest {
                required_coverage_milli: 1_000,
                require_exact_hash: true,
                require_reproducible: true,
                require_accountable_review: true,
                min_independent_approvals: 1,
                max_uncertainty_items: 16,
                reviews: vec![ReleaseReviewAttestation {
                    reviewer_id: "reviewer".into(),
                    role: "independent-researcher".into(),
                    decision: ReleaseReviewDecision::Approve,
                    evidence_digest: ContentHash::of_bytes(b"review"),
                    independent: true,
                }],
            },
            execution_mode: ReleaseExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn release_stage_bridge_preserves_gate_and_publication_boundaries() {
        let mut worker = dry_run_glioma_release_stage_worker(request()).unwrap();
        let stage = GliomaStage {
            stage_id: GliomaStageKind::ResearchObjectRelease.stage_id().into(),
            kind: GliomaStageKind::ResearchObjectRelease,
            depends_on: vec![GliomaStageKind::ReplicationRobustness.stage_id().into()],
            input_schemas: vec![GliomaStageKind::ReplicationRobustness
                .output_schema()
                .into()],
            output_schema: GliomaStageKind::ResearchObjectRelease
                .output_schema()
                .into(),
            required: true,
            readiness: StageReadiness::Ready,
            autonomy_tier: AutonomyTier::A1,
            effects: std::collections::BTreeSet::new(),
            budget_units: 8,
        };
        let input = GliomaStageInput {
            research_id: "bridge-release-research".into(),
            study_id: "bridge-release-study".into(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: Vec::new(),
            source_artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact-main".into(),
                content_hash: ContentHash::of_bytes(b"release-artifact"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            replay_identity: ContentHash::of_bytes(b"stage-replay"),
            attempt: 1,
        };
        let output = worker.execute(&stage, &input).unwrap();
        assert_eq!(output.disposition, GliomaStageDisposition::Completed);
        assert!(!output.uncertainty.is_empty());
        assert!(output
            .negative_evidence
            .iter()
            .any(|value| value.contains("does-not-upload")));
        output.artifact.validate_metadata().unwrap();
    }
}
