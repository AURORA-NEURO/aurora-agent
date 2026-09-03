//! Instrument and robotics preflight program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod calibration;
pub mod execution;
pub mod preflight;

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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InstrumentRobotics;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P08")
}
