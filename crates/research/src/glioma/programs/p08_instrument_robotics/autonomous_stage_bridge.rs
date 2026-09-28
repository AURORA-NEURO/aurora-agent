//! P07 autonomous-engine bridge for the governed P08 instrument operating cycle.
//!
//! The autonomous glioma engine selects an `instrument-preflight` stage as part of its typed
//! dependency graph. This adapter makes that stage executable through the existing P08 preflight,
//! authorization, live-interlock, retry, emergency-stop, and campaign controller. It never
//! converts a successful device operation into biological evidence: the returned stage artifact
//! explicitly carries the operating-cycle result, simulation/effect status, uncertainty, and the
//! requirement for downstream assay adjudication.

use super::execution::{InstrumentExecutionFailure, InstrumentExecutor};
use super::operating_cycle::{
    execute_glioma_instrument_operating_cycle, InstrumentOperatingCycle,
    InstrumentOperatingCycleDisposition, InstrumentOperatingCycleError,
    InstrumentOperatingCycleRequest,
};
use crate::glioma_engine::{
    GliomaStage, GliomaStageDisposition, GliomaStageExecutor, GliomaStageFailure, GliomaStageInput,
    GliomaStageKind, GliomaStageOutput,
};
use bioprism_foundation::TypedResearchArtifact;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// This is a composition of the existing P08-F24 operating-cycle feature and the P07 autonomous
/// stage-worker seam; it intentionally does not consume a second stable catalog slot.
pub const PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P08-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentAutonomousStageBridge1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaInstrumentStageBridgeReceipt {
    pub stage_id: String,
    pub research_id: String,
    pub study_id: String,
    pub operating_cycle: InstrumentOperatingCycle,
    pub simulation_only: bool,
    pub biological_evidence_promoted: bool,
    pub uncertainty: Vec<String>,
    pub next_required_gate: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaInstrumentStageBridgeError {
    #[error("instrument stage bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument operating cycle failed: {0}")]
    OperatingCycle(#[from] InstrumentOperatingCycleError),
}

/// A boxed instrument gateway wrapper keeps the P08 trait object behind a concrete executor type,
/// allowing this worker to be placed in the same `BTreeMap<String, Box<dyn GliomaStageExecutor>>`
/// as computational and evidence workers.
struct DynInstrumentExecutor {
    inner: Box<dyn InstrumentExecutor>,
}

impl InstrumentExecutor for DynInstrumentExecutor {
    fn observe_interlocks(
        &mut self,
    ) -> Result<super::preflight::InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
        self.inner.observe_interlocks()
    }

    fn verify_authorization(
        &mut self,
        authorization: &super::preflight::InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure> {
        self.inner.verify_authorization(authorization)
    }

    fn execute_action(
        &mut self,
        action: &super::preflight::InstrumentAction,
        attempt: u8,
    ) -> Result<super::execution::InstrumentExecutionResult, InstrumentExecutionFailure> {
        self.inner.execute_action(action, attempt)
    }

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
        self.inner.emergency_stop()
    }

    fn simulation_only(&self) -> bool {
        self.inner.simulation_only()
    }
}

/// Institution-local P08 stage worker. The request is preflighted and bound before the worker is
/// inserted into a P07 route. Production callers supply a hardware gateway; tests and MCP use the
/// existing deterministic dry-run gateway.
pub struct GliomaInstrumentStageWorker {
    request: InstrumentOperatingCycleRequest,
    executor: DynInstrumentExecutor,
}

impl GliomaInstrumentStageWorker {
    pub fn new(
        request: InstrumentOperatingCycleRequest,
        executor: Box<dyn InstrumentExecutor>,
    ) -> Result<Self, GliomaInstrumentStageBridgeError> {
        if request.campaign.objective.trim().is_empty() || request.campaign.runs.is_empty() {
            return Err(GliomaInstrumentStageBridgeError::InvalidRequest(
                "instrument operating-cycle objective and at least one run are required".into(),
            ));
        }
        Ok(Self {
            request,
            executor: DynInstrumentExecutor { inner: executor },
        })
    }

    pub fn request(&self) -> &InstrumentOperatingCycleRequest {
        &self.request
    }

    fn execute_cycle(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        if stage.kind != GliomaStageKind::InstrumentPreflight
            || stage.output_schema != GliomaStageKind::InstrumentPreflight.output_schema()
        {
            return Err(GliomaStageFailure {
                reason: "instrument bridge received a non-instrument stage contract".into(),
                retryable: false,
            });
        }
        let cycle = execute_glioma_instrument_operating_cycle(&self.request, &mut self.executor)
            .map_err(|error| GliomaStageFailure {
                reason: error.to_string(),
                retryable: false,
            })?;
        let receipt = GliomaInstrumentStageBridgeReceipt {
            stage_id: stage.stage_id.clone(),
            research_id: input.research_id.clone(),
            study_id: input.study_id.clone(),
            simulation_only: cycle.simulation_only,
            biological_evidence_promoted: false,
            uncertainty: cycle.uncertainty.clone(),
            next_required_gate: "assay-evidence-adjudication-and-qc".into(),
            operating_cycle: cycle.clone(),
        };
        let artifact = TypedResearchArtifact::from_payload(
            format!("instrument-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::to_value(&receipt).map_err(|error| GliomaStageFailure {
                reason: format!("instrument bridge receipt serialization failed: {error}"),
                retryable: false,
            })?,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("instrument bridge artifact digest failed: {error}"),
            retryable: false,
        })?;
        let disposition = match cycle.disposition {
            InstrumentOperatingCycleDisposition::Executed
            | InstrumentOperatingCycleDisposition::Ready => GliomaStageDisposition::Completed,
            InstrumentOperatingCycleDisposition::Negative => GliomaStageDisposition::Negative,
            InstrumentOperatingCycleDisposition::Partial => GliomaStageDisposition::Partial,
            InstrumentOperatingCycleDisposition::Blocked
            | InstrumentOperatingCycleDisposition::Failed
            | InstrumentOperatingCycleDisposition::Unresolved => GliomaStageDisposition::Blocked,
        };
        let mut uncertainty = cycle.uncertainty.clone();
        uncertainty.push("instrument-operation-is-not-biological-evidence".into());
        uncertainty.sort();
        uncertainty.dedup();
        let mut negative_evidence = cycle.negative_evidence.clone();
        negative_evidence.push(
            "instrument-completion-requires-assay-adjudication-before-scientific-claim".into(),
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

impl GliomaStageExecutor for GliomaInstrumentStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        self.execute_cycle(stage, input)
    }
}

/// Construct a worker backed by the deterministic local P08 gateway for sandbox and MCP use.
pub fn dry_run_glioma_instrument_stage_worker(
    request: InstrumentOperatingCycleRequest,
) -> Result<GliomaInstrumentStageWorker, GliomaInstrumentStageBridgeError> {
    let executor = super::operating_cycle::dry_run_instrument_executor_from_request(&request)
        .map_err(GliomaInstrumentStageBridgeError::OperatingCycle)?;
    GliomaInstrumentStageWorker::new(request, Box::new(executor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::{
        preflight_glioma_instrument, CalibrationDisposition, CalibrationPoint, InstrumentAction,
        InstrumentAuthorization, InstrumentCalibration, InstrumentExecutionMode,
        InstrumentInterlockSnapshot, InstrumentOperation, InstrumentParameter,
        InstrumentPreflightRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;
    use std::collections::BTreeSet;

    fn request() -> InstrumentOperatingCycleRequest {
        let action = InstrumentAction {
            action_id: "image-1".into(),
            instrument_id: "imager-1".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 2,
            duration_ticks: 2,
            risk_milli: 10,
            requires_operator: false,
            output_schema: "application/vnd.aurora.glioma.image+json".into(),
            parameters: vec![InstrumentParameter {
                name: "exposure_ms".into(),
                value_milli: 50,
                unit: "ms".into(),
                minimum_milli: Some(1),
                maximum_milli: Some(1_000),
            }],
        };
        let points = vec![
            CalibrationPoint {
                run_id: "cal-1".into(),
                sequence_index: 1,
                observed_milli: 500,
                expected_milli: 500,
                residual_milli: 0,
                drift_from_reference_milli: 0,
                robust_z_milli: 0,
            },
            CalibrationPoint {
                run_id: "cal-2".into(),
                sequence_index: 2,
                observed_milli: 502,
                expected_milli: 500,
                residual_milli: 2,
                drift_from_reference_milli: 2,
                robust_z_milli: 2_000,
            },
            CalibrationPoint {
                run_id: "cal-3".into(),
                sequence_index: 3,
                observed_milli: 504,
                expected_milli: 500,
                residual_milli: 4,
                drift_from_reference_milli: 4,
                robust_z_milli: 4_000,
            },
        ];
        let mut calibration = InstrumentCalibration {
            feature_id: super::super::calibration::FEATURE_ID.into(),
            output_schema: super::super::calibration::OUTPUT_SCHEMA.into(),
            objective: "qualify imager".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            metric_name: "control".into(),
            run_order: vec!["cal-1".into(), "cal-2".into(), "cal-3".into()],
            reference_order: vec!["cal-1".into(), "cal-2".into()],
            points,
            reference_residual_median_milli: 0,
            reference_mad_milli: 0,
            final_drift_milli: 4,
            max_abs_drift_milli: 4,
            slope_milli_per_tick: 2,
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: CalibrationDisposition::Qualified,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        calibration.digest = ContentHash::of_value(&serde_json::json!({
            "objective":"qualify imager", "instrument_id":"imager-1", "model_system":"organoid",
            "metric_name":"control", "run_order":["cal-1","cal-2","cal-3"],
            "reference_order":["cal-1","cal-2"], "points":calibration.points,
            "reference_residual_median_milli":0, "reference_mad_milli":0,
            "final_drift_milli":4, "max_abs_drift_milli":4, "slope_milli_per_tick":2,
            "negative_evidence":[], "uncertainty":[], "disposition":"qualified",
            "feature_id":super::super::calibration::FEATURE_ID,
            "output_schema":super::super::calibration::OUTPUT_SCHEMA
        }))
        .unwrap();
        let interlocks = InstrumentInterlockSnapshot {
            observed_tick: 1,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 1_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100,
            calibration_sequence_index: 1,
        };
        let authorization = InstrumentAuthorization {
            authorization_id: "auth-1".into(),
            operator_id: "operator-1".into(),
            instrument_scope: "imager-1".into(),
            approval_digest: ContentHash::of_bytes(b"approval"),
            issued_tick: 0,
            expires_tick: 100,
            revoked: false,
        };
        let plan = preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: "image glioma organoid invasion".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action.clone()],
            calibration,
            interlocks: interlocks.clone(),
            authorization: authorization.clone(),
            current_tick: 1,
            maximum_total_risk_milli: 100,
            maximum_duration_ticks: 10,
            minimum_waste_capacity_milli: 10,
        })
        .unwrap();
        assert_eq!(
            plan.disposition,
            super::super::preflight::InstrumentPreflightDisposition::Admitted
        );
        InstrumentOperatingCycleRequest {
            campaign: super::super::campaign::InstrumentCampaignRequest {
                objective: "image glioma organoid invasion".into(),
                runs: vec![super::super::campaign::InstrumentCampaignRunRequest {
                    run_id: "run-1".into(),
                    execution: super::super::execution::InstrumentExecutionRequest {
                        objective: "image glioma organoid invasion".into(),
                        plan,
                        actions: vec![action],
                        authorization,
                        live_interlocks: interlocks,
                        current_tick: 1,
                        minimum_waste_capacity_milli: 10,
                        max_retries: 1,
                        require_artifacts: true,
                    },
                }],
                max_runs: 1,
                stop_on_negative: false,
            },
            require_all_admitted: true,
            execution_mode: InstrumentExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn instrument_stage_bridge_executes_p08_and_preserves_non_evidence_boundary() {
        let mut worker = dry_run_glioma_instrument_stage_worker(request()).unwrap();
        let stage = crate::glioma_engine::GliomaStage {
            stage_id: "instrument-preflight".into(),
            kind: GliomaStageKind::InstrumentPreflight,
            input_schemas: vec!["GliomaProtocolSimulation1@1".into()],
            output_schema: GliomaStageKind::InstrumentPreflight.output_schema().into(),
            required: true,
            depends_on: vec!["protocol-simulation".into()],
            readiness: crate::glioma_engine::StageReadiness::Ready,
            autonomy_tier: bioprism_foundation::AutonomyTier::A1,
            effects: BTreeSet::new(),
            budget_units: 8,
        };
        let input = GliomaStageInput {
            research_id: "bridge-research".into(),
            study_id: "bridge-study".into(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: vec![ContentHash::of_bytes(b"protocol")],
            source_artifacts: vec![LocalArtifactRef {
                artifact_id: "source".into(),
                content_hash: ContentHash::of_bytes(b"source"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            replay_identity: ContentHash::of_bytes(b"replay"),
            attempt: 1,
        };
        let output = worker.execute(&stage, &input).unwrap();
        assert_eq!(output.disposition, GliomaStageDisposition::Completed);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "instrument-operation-is-not-biological-evidence"));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("assay-adjudication")));
        assert_eq!(output.artifact.content_type, "GliomaInstrumentPreflight1@2");
        output.artifact.validate_metadata().unwrap();
    }
}
