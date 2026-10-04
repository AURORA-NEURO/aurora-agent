//! Unified autonomous glioma workflow composition.
//!
//! This is the researcher-facing execution seam for the fourteen-stage engine.  It keeps the
//! existing evidence/admission engine as the source of truth, but lets a caller attach the
//! domain algorithms for instrument preflight, reproducible computation, interpretation,
//! replication, research-object release, and federated benchmarking in one run.  Missing optional
//! stage requests intentionally fall back to the deterministic synthetic worker; they are never
//! mistaken for observations or silently promoted into a scientific claim.

use super::evidence_gated_stage_execution::{
    execute_glioma_evidence_gated_stage_engine, GliomaEvidenceGatedStageExecution,
    GliomaEvidenceGatedStageExecutionError, GliomaEvidenceGatedStageExecutionRequest,
};
use crate::glioma::programs::p07_protocol_simulation::stage_worker_registry::DryRunGliomaStageWorker;
use crate::glioma::programs::p08_instrument_robotics::{
    dry_run_glioma_instrument_stage_worker, InstrumentOperatingCycleRequest,
};
use crate::glioma::programs::p09_reproducible_computation::{
    dry_run_glioma_computation_stage_worker, GliomaComputationOperatingCycleRequest,
};
use crate::glioma::programs::p10_interpretation_replication::{
    dry_run_glioma_interpretation_stage_worker, dry_run_glioma_replication_stage_worker,
    GliomaInterpretationOperatingCycleRequest, GliomaReplicationCampaignRequest,
};
use crate::glioma::programs::p11_research_object_release::{
    dry_run_glioma_release_stage_worker, GliomaReleaseOperatingCycleRequest,
};
use crate::glioma::programs::p12_federated_benchmarking::{
    dry_run_glioma_federation_stage_worker, FederatedBenchmarkOperatingCycleRequest,
};
use crate::glioma_engine::{GliomaStageExecutor, GliomaStageKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-autonomous-research-workflow";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchWorkflow1@1";

/// All optional domain-cycle inputs are bound to the same evidence-gated engine run.  A request
/// is local-first: raw payloads remain in the caller's artifact store and only typed cycle
/// contracts cross this composition seam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchWorkflowRequest {
    pub stage_engine: GliomaEvidenceGatedStageExecutionRequest,
    #[serde(default)]
    pub instrument_cycle: Option<InstrumentOperatingCycleRequest>,
    #[serde(default)]
    pub computation_cycle: Option<GliomaComputationOperatingCycleRequest>,
    #[serde(default)]
    pub interpretation_cycle: Option<GliomaInterpretationOperatingCycleRequest>,
    #[serde(default)]
    pub replication_campaign: Option<GliomaReplicationCampaignRequest>,
    #[serde(default)]
    pub release_cycle: Option<GliomaReleaseOperatingCycleRequest>,
    #[serde(default)]
    pub federation_cycle: Option<FederatedBenchmarkOperatingCycleRequest>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaAutonomousResearchWorkflowError {
    #[error("autonomous workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-gated workflow failed: {0}")]
    Stage(#[from] GliomaEvidenceGatedStageExecutionError),
}

fn has_stage(
    profile: &crate::glioma::programs::p07_protocol_simulation::GliomaStageWorkerProfile,
    kind: GliomaStageKind,
) -> bool {
    profile.stage_kinds.len() == 1
        && profile.stage_kinds.contains(&kind)
        && profile
            .output_schemas
            .contains(&kind.output_schema().to_string())
}

/// Execute the unified workflow with deterministic institution-local workers for every supplied
/// domain cycle.  This function performs no network, instrument, federation, or clinical action;
/// production hosts replace the dry-run workers behind the same typed stage-executor contracts.
pub fn execute_glioma_autonomous_research_workflow_dry_run(
    request: &GliomaAutonomousResearchWorkflowRequest,
) -> Result<GliomaEvidenceGatedStageExecution, GliomaAutonomousResearchWorkflowError> {
    if request.stage_engine.workers.is_empty() {
        return Err(GliomaAutonomousResearchWorkflowError::InvalidRequest(
            "at least one local stage-worker profile is required".into(),
        ));
    }
    let mut workers: BTreeMap<String, Box<dyn GliomaStageExecutor>> = BTreeMap::new();
    for profile in &request.stage_engine.workers {
        if has_stage(profile, GliomaStageKind::InstrumentPreflight) {
            if let Some(cycle) = &request.instrument_cycle {
                let worker =
                    dry_run_glioma_instrument_stage_worker(cycle.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        if has_stage(profile, GliomaStageKind::ComputationalExecution) {
            if let Some(cycle) = &request.computation_cycle {
                let worker =
                    dry_run_glioma_computation_stage_worker(cycle.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        if has_stage(profile, GliomaStageKind::StatisticalInterpretation) {
            if let Some(cycle) = &request.interpretation_cycle {
                let worker =
                    dry_run_glioma_interpretation_stage_worker(cycle.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        if has_stage(profile, GliomaStageKind::ReplicationRobustness) {
            if let Some(campaign) = &request.replication_campaign {
                let worker =
                    dry_run_glioma_replication_stage_worker(campaign.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        if has_stage(profile, GliomaStageKind::ResearchObjectRelease) {
            if let Some(cycle) = &request.release_cycle {
                let worker =
                    dry_run_glioma_release_stage_worker(cycle.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        if has_stage(profile, GliomaStageKind::FederationBenchmarking) {
            if let Some(cycle) = &request.federation_cycle {
                let worker =
                    dry_run_glioma_federation_stage_worker(cycle.clone()).map_err(|error| {
                        GliomaAutonomousResearchWorkflowError::InvalidRequest(error.to_string())
                    })?;
                workers.insert(profile.worker_id.clone(), Box::new(worker));
                continue;
            }
        }
        workers.insert(profile.worker_id.clone(), Box::new(DryRunGliomaStageWorker));
    }
    execute_glioma_evidence_gated_stage_engine(&request.stage_engine, workers)
        .map_err(GliomaAutonomousResearchWorkflowError::Stage)
}
