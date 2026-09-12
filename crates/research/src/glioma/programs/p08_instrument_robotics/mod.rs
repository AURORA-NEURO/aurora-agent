//! Instrument and robotics preflight program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod assay_adjudication;
pub mod calibration;
pub mod campaign;
pub mod execution;
pub mod fleet_scheduler;
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

pub use campaign::{
    execute_glioma_instrument_campaign, InstrumentCampaign, InstrumentCampaignDisposition,
    InstrumentCampaignError, InstrumentCampaignFailure, InstrumentCampaignRequest,
    InstrumentCampaignRunRequest, InstrumentCampaignRunResult, InstrumentCampaignStopReason,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InstrumentRobotics;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P08")
}
