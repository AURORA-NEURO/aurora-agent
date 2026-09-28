//! Instrument and robotics preflight program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod acquisition_capacity_controller;
pub mod adaptive_campaign;
pub mod adaptive_microscopy;
pub mod assay_adjudication;
pub mod assay_provenance_integrity_audit;
pub mod assay_run_schema;
pub mod autonomous_stage_bridge;
pub mod batch_stability;
pub mod calibration;
pub mod campaign;
pub mod cross_site_protocol_conformance;
pub mod execution;
pub mod federated_consensus;
pub mod federated_device_capability_manifest;
pub mod federated_instrument_operations;
pub mod fleet_execution;
pub mod fleet_health_monitor;
pub mod fleet_scheduler;
pub mod high_throughput_acquisition_console;
pub mod instrument_operator_approval_console;
pub mod instrument_protocol_compiler;
pub mod maintenance_window_manager;
pub mod multichannel_concordance;
pub mod operating_cycle;
pub mod phase_resolved_invasion_schedule;
pub mod preflight;
pub mod protocol_binding;
pub mod recovery;
pub mod research_frontier;
pub mod science_loop;
pub mod signal_extraction;
pub mod simulated_protocol_workflow;
pub mod synchronized_multimodal_acquisition;

pub use assay_adjudication::{
    AssayEvidenceDisposition, AssayEvidenceError, AssayEvidenceObservation, AssayEvidenceRecord,
    AssayEvidenceRequest, InstrumentAssayEvidenceAssessment, adjudicate_glioma_assay_evidence,
};
pub use assay_provenance_integrity_audit::{
    audit_glioma_assay_provenance, AssayLifecycleStatus, AssayProvenanceAuditError,
    AssayProvenanceAuditRequest, AssayProvenanceAuditResult, AssayProvenanceDisposition,
    AssayProvenanceIntegrityAudit, AssayProvenanceRun,
};
pub use assay_run_schema::{
    compile_glioma_assay_run_spec, record_glioma_assay_run_result, AssayAcquisitionConfiguration,
    AssayAcquisitionMode, AssayChannelObservation, AssayChannelSpec, AssayChannelState,
    AssayMaterialKind, AssayProtocolReference, AssayRunDisposition, AssayRunResult,
    AssayRunSchemaError, AssayRunSpec, AssayRunSpecRequest, AssaySampleToken,
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
pub use instrument_protocol_compiler::{
    compile_glioma_instrument_protocol, CompensationAction, CompiledInstrumentProtocol,
    CompiledProtocolDisposition, ExpectedProtocolArtifact, InstrumentPreflightChecklistItem,
    InstrumentProtocolCompileRequest, InstrumentProtocolCompilerError, PreflightCheck,
    ProtocolInterlockPolicy, ProtocolParameter, ProtocolStep, ProtocolUnit,
    UnsupportedProtocolStep, UnsupportedStepDisposition,
};

pub use phase_resolved_invasion_schedule::{
    compile_glioma_phase_resolved_invasion_actions, design_glioma_phase_resolved_invasion_schedule,
    CellCyclePhaseDwellPrior, GliomaMotilityTimescalePrior, PhaseResolvedAcquisitionProfile,
    PhaseResolvedInstrumentActionTemplate, PhaseResolvedInvasionActionPlan,
    PhaseResolvedInvasionSchedule, PhaseResolvedInvasionScheduleRequest,
    PhaseResolvedProfileDisposition, PhaseResolvedProfileScore, PhaseResolvedScheduleDisposition,
    PhaseResolvedScheduleError, PhaseSamplingResolution, ACTION_PLAN_SCHEMA,
};

pub use adaptive_campaign::{
    AdaptiveInstrumentCampaign, AdaptiveInstrumentCampaignDisposition,
    AdaptiveInstrumentCampaignError, AdaptiveInstrumentCampaignRequest,
    AdaptiveInstrumentCandidate, AdaptiveInstrumentDecision, dry_run_adaptive_instrument_executor,
    execute_glioma_adaptive_instrument_campaign,
};

pub use adaptive_microscopy::{
    build_adaptive_microscopy_candidate_from_morphodynamics,
    execute_glioma_adaptive_microscopy_research_workflow, execute_glioma_adaptive_microscopy_round,
    AdaptiveMicroscopyCandidate, AdaptiveMicroscopyDisposition, AdaptiveMicroscopyError,
    AdaptiveMicroscopyExecutionBinding, AdaptiveMicroscopyRequest,
    AdaptiveMicroscopyResearchStopReason, AdaptiveMicroscopyResearchWorkflowError,
    AdaptiveMicroscopyResearchWorkflowRequest, AdaptiveMicroscopyResearchWorkflowRun,
    AdaptiveMicroscopyRound, AdaptiveMicroscopyStopReason, GliomaMicroscopyMaterial,
    MicroscopyCandidateDecision, MicroscopyCandidateDisposition, MicroscopyDoseCalibration,
    MicroscopyOutcomeLikelihood, MicroscopyStateProbability, MicroscopyStratumTarget,
};

pub use autonomous_stage_bridge::{
    dry_run_glioma_instrument_stage_worker, GliomaInstrumentStageBridgeError,
    GliomaInstrumentStageBridgeReceipt, GliomaInstrumentStageWorker,
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

pub use protocol_binding::{
    compile_glioma_instrument_protocol_binding, BoundInstrumentAction, BoundInstrumentCommand,
    BoundInstrumentParameter, InstrumentCommandCapability, InstrumentCommandParameter,
    InstrumentControlProtocol, InstrumentProtocolBindingError, InstrumentProtocolBindingPlan,
    InstrumentProtocolGateway, InstrumentProtocolManifest, ProtocolBoundInstrumentExecutor,
};

pub use simulated_protocol_workflow::{
    execute_glioma_simulation_gated_instrument_workflow, CompletedProtocolPrerequisite,
    SimulationGatedInstrumentWorkflowDisposition, SimulationGatedInstrumentWorkflowError,
    SimulationGatedInstrumentWorkflowRequest, SimulationGatedInstrumentWorkflowRun,
    SimulationGatedInstrumentWorkflowStopReason,
};

pub use synchronized_multimodal_acquisition::{
    execute_glioma_synchronized_multimodal_acquisition, DryRunSynchronizedAcquisitionExecutor,
    InstrumentClockCalibration, SynchronizedAcquisitionChannel, SynchronizedAcquisitionDisposition,
    SynchronizedAcquisitionError, SynchronizedAcquisitionExecutionFailure,
    SynchronizedAcquisitionExecutor, SynchronizedAcquisitionRequest,
    SynchronizedAcquisitionStopReason, SynchronizedCapture, SynchronizedCaptureAlignment,
    SynchronizedCaptureDisposition, SynchronizedCaptureOutcome, SynchronizedMultimodalAcquisition,
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

pub use acquisition_capacity_controller::{
    plan_glioma_acquisition_capacity, AcquisitionAllocation, AcquisitionCapacityError,
    AcquisitionCapacityPlan, AcquisitionCapacityRequest, AcquisitionCapacityResource,
    AcquisitionDeferral, AcquisitionDemand,
};
pub use cross_site_protocol_conformance::{
    assess_glioma_cross_site_protocol_conformance, ProtocolConformanceError,
    ProtocolConformanceMatrix, ProtocolConformanceRequest, ProtocolConformanceStatus,
    ProtocolStepSpec, ReferenceProtocol, SiteConformanceResult, SiteProtocolRealization,
};
pub use federated_device_capability_manifest::{
    publish_glioma_federated_device_capability_manifest, AvailabilityWindow,
    CapabilityCompatibility, DataLocality, DeviceCapabilityClaim,
    FederatedDeviceCapabilityManifest, FederatedDeviceCapabilityRequest,
    FederatedDeviceManifestError, ManifestAvailabilityState,
};
pub use federated_instrument_operations::{
    exchange_glioma_federated_instrument_operations, FederatedInstrumentOperationsError,
    FederatedInstrumentOperationsRequest, FederatedInstrumentOperationsSnapshot,
    FederatedInstrumentSiteSummary, SiteOperationsResult, SiteOperationsState,
};
pub use fleet_health_monitor::{
    monitor_glioma_instrument_fleet_health, FleetHealthAssessment, FleetHealthDisposition,
    FleetHealthError, FleetHealthMonitorRequest, InstrumentHealthAssessment,
    InstrumentHealthObservation,
};
pub use high_throughput_acquisition_console::{
    plan_glioma_acquisition_operations, AcquisitionAssignmentView, AcquisitionDeviceSummary,
    AcquisitionOperationsError, AcquisitionOperationsRequest, AcquisitionOperationsSnapshot,
    AcquisitionQueueItem, PreflightState, QueueRisk, ReorderProposal,
};
pub use instrument_operator_approval_console::{
    approve_glioma_instrument_action, ApprovalDisposition, InterlockObservation, InterlockState,
    OperatorApproval, OperatorApprovalError, OperatorApprovalRequest,
};
pub use maintenance_window_manager::{
    plan_glioma_instrument_maintenance, InstrumentMaintenancePlan, InstrumentMaintenanceWindow,
    InstrumentReservation, MaintenanceDevice, MaintenanceWindowDisposition, MaintenanceWindowError,
    MaintenanceWindowRequest,
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
