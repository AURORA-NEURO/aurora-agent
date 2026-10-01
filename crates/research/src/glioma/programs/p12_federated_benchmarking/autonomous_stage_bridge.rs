//! P07 autonomous-engine bridge for the aggregate-only P12 federation cycle.
//!
//! This worker makes the terminal stage of a glioma program executable while keeping the trust
//! boundary explicit: sites retain raw traces, the campaign exchanges typed aggregates, and a
//! qualified benchmark is still a consortium-governance result rather than a clinical decision.

use super::campaign::{
    DryRunFederatedBenchmarkCampaignExecutor, FederatedBenchmarkAction,
    FederatedBenchmarkCampaignExecutor, FederatedBenchmarkExecutionFailure,
};
use super::consensus::{FederatedBenchmarkRequest, FederatedBenchmarkSite};
use super::operating_cycle::{
    execute_federated_benchmark_operating_cycle, FederatedBenchmarkOperatingCycle,
    FederatedBenchmarkOperatingCycleDisposition, FederatedBenchmarkOperatingCycleError,
    FederatedBenchmarkOperatingCycleRequest,
};
use crate::glioma_engine::{
    GliomaStage, GliomaStageDisposition, GliomaStageExecutor, GliomaStageFailure, GliomaStageInput,
    GliomaStageKind, GliomaStageOutput,
};
use bioprism_foundation::TypedResearchArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P12-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaFederationAutonomousStageBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFederationStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub operating_cycle: FederatedBenchmarkOperatingCycle,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaFederationStageBridgeError {
    #[error("federation stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federation operating cycle failed: {0}")]
    OperatingCycle(#[from] FederatedBenchmarkOperatingCycleError),
}

struct DynFederatedBenchmarkExecutor {
    inner: Box<dyn FederatedBenchmarkCampaignExecutor>,
}

impl FederatedBenchmarkCampaignExecutor for DynFederatedBenchmarkExecutor {
    fn execute_action(
        &mut self,
        action: &FederatedBenchmarkAction,
        request: &FederatedBenchmarkRequest,
        attempt: u8,
    ) -> Result<FederatedBenchmarkSite, FederatedBenchmarkExecutionFailure> {
        self.inner.execute_action(action, request, attempt)
    }
}

pub struct GliomaFederationStageWorker {
    request: FederatedBenchmarkOperatingCycleRequest,
    executor: DynFederatedBenchmarkExecutor,
}

impl GliomaFederationStageWorker {
    pub fn new(
        request: FederatedBenchmarkOperatingCycleRequest,
        executor: Box<dyn FederatedBenchmarkCampaignExecutor>,
    ) -> Result<Self, GliomaFederationStageBridgeError> {
        if request.campaign.benchmark.objective.trim().is_empty()
            || request.campaign.initial_sites.is_empty()
            || request.campaign.budget_units == 0
            || request.campaign.max_rounds == 0
        {
            return Err(GliomaFederationStageBridgeError::InvalidRequest(
                "federation objective, aggregate site, budget, and round bound are required".into(),
            ));
        }
        Ok(Self {
            request,
            executor: DynFederatedBenchmarkExecutor { inner: executor },
        })
    }

    pub fn request(&self) -> &FederatedBenchmarkOperatingCycleRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::FederationBenchmarking
            || stage.output_schema != GliomaStageKind::FederationBenchmarking.output_schema()
        {
            return Err(GliomaStageFailure {
                reason: "federation bridge received a non-federation stage contract".into(),
                retryable: false,
            });
        }
        let cycle = execute_federated_benchmark_operating_cycle(&self.request, &mut self.executor)
            .map_err(|error| GliomaStageFailure {
                reason: error.to_string(),
                retryable: false,
            })?;
        let receipt = GliomaFederationStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: cycle.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: cycle.uncertainty.clone(),
            next_required_gate: "consortium-governance-and-independent-validation".into(),
            operating_cycle: cycle.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("federation-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("federation bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("federation bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match cycle.disposition {
            FederatedBenchmarkOperatingCycleDisposition::Qualified => {
                GliomaStageDisposition::Completed
            }
            FederatedBenchmarkOperatingCycleDisposition::Negative => {
                GliomaStageDisposition::Negative
            }
            FederatedBenchmarkOperatingCycleDisposition::Heterogeneous
            | FederatedBenchmarkOperatingCycleDisposition::Partial => {
                GliomaStageDisposition::Partial
            }
            FederatedBenchmarkOperatingCycleDisposition::Blocked
            | FederatedBenchmarkOperatingCycleDisposition::Unresolved => {
                GliomaStageDisposition::Blocked
            }
        };
        let mut uncertainty = cycle.uncertainty.clone();
        uncertainty.push(
            "federated-benchmark-requires-consortium-governance-and-independent-validation".into(),
        );
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = cycle.negative_evidence.clone();
        negative_evidence.push("federation-exchanges-aggregate-artifacts-only".into());
        negative_evidence.push("federated-result-does-not-authorize-clinical-decision".into());
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

impl GliomaStageExecutor for GliomaFederationStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

pub fn dry_run_glioma_federation_stage_worker(
    request: FederatedBenchmarkOperatingCycleRequest,
) -> Result<GliomaFederationStageWorker, GliomaFederationStageBridgeError> {
    GliomaFederationStageWorker::new(request, Box::new(DryRunFederatedBenchmarkCampaignExecutor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p12_federated_benchmarking::{
        FederatedBenchmarkActionKind, FederatedBenchmarkCampaignRequest, FederatedBenchmarkRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef, StageReadiness};
    use bioprism_foundation::AutonomyTier;
    use bioprism_ids::ContentHash;

    fn request() -> FederatedBenchmarkOperatingCycleRequest {
        let benchmark = FederatedBenchmarkRequest {
            objective: "compare organoid invasion model transportability".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 2,
            effect_threshold_milli: 25,
            max_i2_milli: 500,
            min_signal_to_noise_milli: 100,
            max_site_spread_milli: 500,
            max_leave_one_out_shift_milli: 500,
        };
        let site = FederatedBenchmarkSite {
            site_id: "seed-site".into(),
            study_id: "seed-study".into(),
            capability_id: benchmark.capability_id.clone(),
            benchmark_world: benchmark.benchmark_world.clone(),
            metric_name: benchmark.metric_name.clone(),
            model_system: benchmark.model_system,
            artifact: LocalArtifactRef {
                artifact_id: "seed-aggregate".into(),
                content_hash: ContentHash::of_bytes(b"seed-aggregate"),
                content_type: "application/vnd.aurora.glioma.aggregate+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: 620,
            uncertainty_milli: 40,
            replicate_count: 3,
        };
        FederatedBenchmarkOperatingCycleRequest {
            campaign: FederatedBenchmarkCampaignRequest {
                benchmark,
                initial_sites: vec![site],
                actions: vec![super::super::campaign::FederatedBenchmarkAction {
                    action_id: "expand-site".into(),
                    kind: FederatedBenchmarkActionKind::ExpandCoverage,
                    target_site_id: None,
                    cost_units: 1,
                    expected_information_milli: 900,
                    expected_effect_milli: 100,
                    feasibility_milli: 900,
                    risk_milli: 50,
                    requested_replicates: 3,
                }],
                budget_units: 1,
                max_rounds: 2,
                max_retries: 1,
                stop_on_qualified: false,
                stop_on_negative: false,
            },
            execution_mode:
                super::super::operating_cycle::FederatedBenchmarkExecutionMode::LocalSimulation,
            require_aggregate_only: true,
        }
    }

    #[test]
    fn federation_stage_bridge_preserves_aggregate_only_boundary_when_coverage_is_blocked() {
        let mut worker = dry_run_glioma_federation_stage_worker(request()).unwrap();
        let stage = GliomaStage {
            stage_id: GliomaStageKind::FederationBenchmarking.stage_id().into(),
            kind: GliomaStageKind::FederationBenchmarking,
            depends_on: vec![GliomaStageKind::ResearchObjectRelease.stage_id().into()],
            input_schemas: vec![GliomaStageKind::ResearchObjectRelease
                .output_schema()
                .into()],
            output_schema: GliomaStageKind::FederationBenchmarking
                .output_schema()
                .into(),
            required: true,
            readiness: StageReadiness::Ready,
            autonomy_tier: AutonomyTier::A1,
            effects: std::collections::BTreeSet::new(),
            budget_units: 16,
        };
        let input = GliomaStageInput {
            research_id: "bridge-federation-research".into(),
            study_id: "bridge-federation-study".into(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: Vec::new(),
            source_artifacts: Vec::new(),
            replay_identity: ContentHash::of_bytes(b"federation-stage-replay"),
            attempt: 1,
        };
        let output = worker.execute(&stage, &input).unwrap();
        assert_eq!(output.disposition, GliomaStageDisposition::Blocked);
        assert!(output
            .negative_evidence
            .iter()
            .any(|value| value.contains("aggregate-artifacts-only")));
        assert!(output
            .uncertainty
            .iter()
            .any(|value| value.contains("consortium-governance")));
        output.artifact.validate_metadata().unwrap();
    }
}
