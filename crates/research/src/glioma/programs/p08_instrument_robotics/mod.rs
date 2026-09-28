//! Instrument and robotics preflight program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod adaptive_campaign;
pub mod assay_adjudication;
pub mod batch_stability;
pub mod calibration;
pub mod campaign;
pub mod execution;
pub mod federated_consensus;
pub mod fleet_execution;
pub mod fleet_scheduler;
pub mod multichannel_concordance;
pub mod operating_cycle;
pub mod preflight;
pub mod recovery;
pub mod research_frontier;
pub mod science_loop;
pub mod signal_extraction;

pub use assay_adjudication::{
    AssayEvidenceDisposition, AssayEvidenceError, AssayEvidenceObservation, AssayEvidenceRecord,
    AssayEvidenceRequest, InstrumentAssayEvidenceAssessment, adjudicate_glioma_assay_evidence,
};
pub use batch_stability::{
    BatchStabilityDisposition, BatchStabilityError, ChannelStability, ChannelStabilityDisposition,
    InstrumentBatchStability, InstrumentSignalRun, SignalBatchStabilityRequest,
    analyze_glioma_instrument_batch_stability,
};
pub use federated_consensus::{
    FederatedConsensusDisposition, FederatedConsensusError, FederatedEndpointConsensus,
    FederatedEndpointDisposition, FederatedEndpointValue, FederatedInstrumentConsensus,
    FederatedInstrumentConsensusRequest, FederatedInstrumentSite,
    analyze_glioma_federated_instrument_consensus,
};

pub use adaptive_campaign::{
    AdaptiveInstrumentCampaign, AdaptiveInstrumentCampaignDisposition,
    AdaptiveInstrumentCampaignError, AdaptiveInstrumentCampaignRequest,
    AdaptiveInstrumentCandidate, AdaptiveInstrumentDecision, dry_run_adaptive_instrument_executor,
    execute_glioma_adaptive_instrument_campaign,
};

pub use calibration::{
    CalibrationDisposition, CalibrationError, CalibrationPoint, CalibrationRequest, CalibrationRun,
    InstrumentCalibration, analyze_instrument_calibration,
};

pub use preflight::{
    InstrumentAction, InstrumentActionDecision, InstrumentActionDisposition,
    InstrumentAuthorization, InstrumentInterlockSnapshot, InstrumentOperation, InstrumentParameter,
    InstrumentPreflightDisposition, InstrumentPreflightError, InstrumentPreflightPlan,
    InstrumentPreflightRequest, preflight_glioma_instrument,
};

pub use recovery::{
    InstrumentRecoveryAction, InstrumentRecoveryDecision, InstrumentRecoveryDisposition,
    InstrumentRecoveryError, InstrumentRecoveryPlan, InstrumentRecoveryPriority,
    InstrumentRecoveryRequest, plan_glioma_instrument_recovery,
};

pub use execution::{
    DryRunInstrumentExecutor, InstrumentExecutionDisposition, InstrumentExecutionError,
    InstrumentExecutionFailure, InstrumentExecutionRequest, InstrumentExecutionResult,
    InstrumentExecutionRun, InstrumentExecutionStopReason, InstrumentExecutor,
    execute_glioma_instrument_plan,
};

pub use fleet_scheduler::{
    InstrumentFleetAssignment, InstrumentFleetBlockedTask, InstrumentFleetDisposition,
    InstrumentFleetResource, InstrumentFleetSchedule, InstrumentFleetScheduleRequest,
    InstrumentFleetSchedulerError, InstrumentFleetTask, InstrumentFleetUtilization,
    schedule_glioma_instrument_fleet,
};

pub use fleet_execution::{
    InstrumentFleetExecution, InstrumentFleetExecutionDisposition, InstrumentFleetExecutionError,
    InstrumentFleetExecutionFailure, InstrumentFleetExecutionRequest,
    InstrumentFleetExecutionResult, InstrumentFleetExecutionRunRequest,
    InstrumentFleetExecutionStopReason, execute_glioma_instrument_fleet,
};

pub use campaign::{
    InstrumentCampaign, InstrumentCampaignDisposition, InstrumentCampaignError,
    InstrumentCampaignFailure, InstrumentCampaignRequest, InstrumentCampaignRunRequest,
    InstrumentCampaignRunResult, InstrumentCampaignStopReason, execute_glioma_instrument_campaign,
};

pub use operating_cycle::{
    InstrumentExecutionMode, InstrumentOperatingCycle, InstrumentOperatingCycleDisposition,
    InstrumentOperatingCycleError, InstrumentOperatingCycleRequest, InstrumentPreflightSummary,
    dry_run_instrument_executor_from_request, execute_glioma_instrument_operating_cycle,
};

pub use multichannel_concordance::{
    ChannelConcordance, ChannelConcordanceDisposition, InstrumentMultichannelConcordance,
    MultichannelConcordanceDisposition, MultichannelConcordanceError,
    MultichannelConcordanceRequest, MultichannelInput, MultichannelPoint,
    analyze_glioma_instrument_multichannel_concordance,
};
pub use research_frontier::{
    InstrumentResearchFrontier, InstrumentResearchFrontierDisposition,
    InstrumentResearchFrontierError, InstrumentResearchFrontierRequest,
    InstrumentResearchFrontierRun, compile_glioma_instrument_research_frontier,
    execute_glioma_instrument_research_frontier,
};
pub use science_loop::{
    InstrumentAssayEvidenceRunObservation, InstrumentScienceLoop, InstrumentScienceLoopAssessment,
    InstrumentScienceLoopDisposition, InstrumentScienceLoopError, InstrumentScienceLoopRequest,
    execute_glioma_instrument_science_loop,
};
pub use signal_extraction::{
    InstrumentSignalChannel, InstrumentSignalExtraction, InstrumentSignalPeak,
    InstrumentSignalPoint, SignalChannelDisposition, SignalExtractionDisposition,
    SignalExtractionError, SignalExtractionRequest, extract_glioma_instrument_signal,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InstrumentRobotics;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P08")
}
