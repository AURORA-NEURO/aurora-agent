//! Instrument and robotics preflight program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod assay_adjudication;
pub mod calibration;
pub mod campaign;
pub mod execution;
pub mod fleet_execution;
pub mod fleet_scheduler;
pub mod operating_cycle;
pub mod preflight;

pub use assay_adjudication::{
    adjudicate_glioma_assay_evidence, AssayEvidenceDisposition, AssayEvidenceError,
    AssayEvidenceObservation, AssayEvidenceRecord, AssayEvidenceRequest,
    InstrumentAssayEvidenceAssessment,
};

pub use calibration::{
    analyze_instrument_calibration, CalibrationDisposition, CalibrationError, CalibrationPoint,
    CalibrationRequest, CalibrationRun, InstrumentCalibration,
};

pub use preflight::{
    preflight_glioma_instrument, InstrumentAction, InstrumentActionDecision,
    InstrumentActionDisposition, InstrumentAuthorization, InstrumentInterlockSnapshot,
    InstrumentOperation, InstrumentParameter, InstrumentPreflightDisposition,
    InstrumentPreflightError, InstrumentPreflightPlan, InstrumentPreflightRequest,
};

pub use execution::{
    execute_glioma_instrument_plan, DryRunInstrumentExecutor, InstrumentExecutionDisposition,
    InstrumentExecutionError, InstrumentExecutionFailure, InstrumentExecutionRequest,
    InstrumentExecutionResult, InstrumentExecutionRun, InstrumentExecutionStopReason,
    InstrumentExecutor,
};

pub use fleet_scheduler::{
    schedule_glioma_instrument_fleet, InstrumentFleetAssignment, InstrumentFleetBlockedTask,
    InstrumentFleetDisposition, InstrumentFleetResource, InstrumentFleetSchedule,
    InstrumentFleetScheduleRequest, InstrumentFleetSchedulerError, InstrumentFleetTask,
    InstrumentFleetUtilization,
};

pub use fleet_execution::{
    execute_glioma_instrument_fleet, InstrumentFleetExecution, InstrumentFleetExecutionDisposition,
    InstrumentFleetExecutionError, InstrumentFleetExecutionFailure,
    InstrumentFleetExecutionRequest, InstrumentFleetExecutionResult,
    InstrumentFleetExecutionRunRequest, InstrumentFleetExecutionStopReason,
};

pub use campaign::{
    execute_glioma_instrument_campaign, InstrumentCampaign, InstrumentCampaignDisposition,
    InstrumentCampaignError, InstrumentCampaignFailure, InstrumentCampaignRequest,
    InstrumentCampaignRunRequest, InstrumentCampaignRunResult, InstrumentCampaignStopReason,
};

pub use operating_cycle::{
    dry_run_instrument_executor_from_request, execute_glioma_instrument_operating_cycle,
    InstrumentExecutionMode, InstrumentOperatingCycle, InstrumentOperatingCycleDisposition,
    InstrumentOperatingCycleError, InstrumentOperatingCycleRequest, InstrumentPreflightSummary,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InstrumentRobotics;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P08")
}
