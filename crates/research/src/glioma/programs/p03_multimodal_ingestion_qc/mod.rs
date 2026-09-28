//! Multimodal ingestion and quality-control program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod campaign;
pub mod concordance;
pub mod consensus;
pub mod contradiction_adjudication;
pub mod decision_gate;
pub mod drift_surveillance;
pub mod dropout_stress;
pub mod evidence_fusion;
pub mod graph_fusion;
pub mod harmonization;
pub mod ingestion_manifest;
pub mod latent_factors;
pub mod microscopy_morphodynamics;
pub mod missingness_audit;
pub mod modality_portfolio;
pub mod operating_cycle;
pub mod prospective_quality;
pub mod quality_adaptive_campaign;
pub mod quality_execution;
pub mod quality_recovery;
pub mod quality_remediation;
pub mod quality_root_cause;
pub mod quality_scheduler;
pub mod quality_transport;
pub mod readiness_gate;
pub mod reliability_calibration;
pub mod sensitivity;
pub mod spatial_communication;
pub mod spatial_niche;
pub mod spatial_propagation;
pub mod spatial_registration;
pub mod temporal_fusion;
pub mod temporal_spatial_alignment;

pub use campaign::{
    DryRunMultimodalIngestionCampaignExecutor, IngestionQcAction, IngestionQcActionKind,
    MultimodalIngestionCampaign, MultimodalIngestionCampaignDisposition,
    MultimodalIngestionCampaignError, MultimodalIngestionCampaignExecutor,
    MultimodalIngestionCampaignRequest, MultimodalIngestionCampaignRound,
    MultimodalIngestionCampaignStopReason, MultimodalIngestionExecutionFailure,
    execute_glioma_multimodal_ingestion_campaign,
};
pub use concordance::{
    ConcordanceDisposition, ConcordanceError, ConcordanceRequest, FeatureValue,
    ModalityConcordance, ModalityVector, MultimodalConcordance, PairConcordanceDisposition,
    analyze_multimodal_concordance,
};
pub use consensus::{
    ConsensusAssignment, ConsensusCluster, ConsensusDisposition, ConsensusError, ConsensusRequest,
    MultimodalConsensus, analyze_multimodal_consensus,
};
pub use contradiction_adjudication::{
    ContradictionAdjudication, ContradictionAdjudicationDisposition,
    ContradictionAdjudicationError, ContradictionAdjudicationRequest, ContradictionKind,
    ModalityPairAdjudication, adjudicate_glioma_multimodal_contradictions,
};
pub use decision_gate::{
    DecisionDirection, DecisionGateModalityObservation, MultimodalDecisionGateAnalysis,
    MultimodalDecisionGateDisposition, MultimodalDecisionGateError, MultimodalDecisionGateRequest,
    analyze_glioma_multimodal_decision_gate,
};
pub use drift_surveillance::{
    DriftDisposition, DriftMetricSummary, DriftObservation, DriftSurveillance,
    DriftSurveillanceError, DriftSurveillanceRequest, surveil_glioma_multimodal_drift,
};
pub use dropout_stress::{
    DropoutModalitySignal, DropoutScenario, DropoutScenarioDisposition, DropoutScenarioResult,
    DropoutStressAnalysis, DropoutStressDisposition, DropoutStressError, DropoutStressRequest,
    analyze_glioma_multimodal_dropout_stress,
};
pub use evidence_fusion::{
    EndpointEvidence, EvidenceContribution, EvidenceFusionAnalysis, EvidenceFusionDisposition,
    EvidenceFusionError, EvidenceFusionRequest, analyze_glioma_multimodal_evidence_fusion,
};
pub use graph_fusion::{
    GraphFusionAnalysis, GraphFusionDisposition, GraphFusionError, GraphFusionNeighbour,
    GraphFusionRequest, GraphFusionState, GraphFusionVector,
    analyze_glioma_multimodal_graph_fusion,
};
pub use harmonization::{
    BatchHarmonizationDiagnostic, HarmonizationDisposition, HarmonizationError,
    HarmonizationRequest, HarmonizationVector, HarmonizedFeature, HarmonizedVector,
    MultimodalHarmonization, harmonize_glioma_multimodal_batches,
};
pub use ingestion_manifest::{
    IngestionManifestDisposition, IngestionManifestError, MultimodalIngestionItem,
    MultimodalIngestionManifest, MultimodalIngestionManifestRequest,
    build_glioma_multimodal_ingestion_manifest,
};
pub use latent_factors::{
    LatentFactorAnalysis, LatentFactorComponent, LatentFactorDisposition, LatentFactorError,
    LatentFactorRequest, LatentFactorVector, LatentLoading, LatentScore,
    analyze_glioma_latent_factors,
};
pub use microscopy_morphodynamics::{
    analyze_glioma_microscopy_morphodynamics, CellTrackFrame, GliomaMicroscopyMaterial,
    LabeledMorphodynamicField, MicroscopyFieldInput, MicroscopyMorphodynamicAnalysis,
    MicroscopyMorphodynamicError, MicroscopyMorphodynamicRequest, MicroscopyOutcomeLikelihood,
    MicroscopyStateProbability, MorphodynamicFieldDisposition, MorphodynamicFieldEstimate,
    MorphodynamicModelDisposition, StateValidationSummary, TrackedGliomaCell,
};
pub use missingness_audit::{
    MissingnessAudit, MissingnessAuditDisposition, MissingnessAuditError, MissingnessAuditRequest,
    MissingnessModalitySummary, MissingnessObservation, MissingnessPairSummary, MissingnessPattern,
    MissingnessPatternDisposition, MissingnessState, analyze_glioma_multimodal_missingness,
};
pub use modality_portfolio::{
    ModalityCapability, ModalityPortfolioAlternative, ModalityPortfolioDisposition,
    ModalityPortfolioError, ModalityPortfolioPlan, ModalityPortfolioRequest,
    plan_glioma_multimodal_portfolio,
};
pub use operating_cycle::{
    GliomaMultimodalOperatingCycle, GliomaMultimodalOperatingCycleDisposition,
    GliomaMultimodalOperatingCycleError, GliomaMultimodalOperatingCycleRequest,
    MultimodalExecutionMode, execute_glioma_multimodal_operating_cycle,
    execute_glioma_multimodal_operating_cycle_dry_run,
};
pub use prospective_quality::{
    ModalityQualityForecast, ProspectiveQualityError, ProspectiveQualityForecast,
    ProspectiveQualityRequest, QualityForecastDisposition, QualityForecastObservation,
    forecast_glioma_multimodal_quality,
};
pub use quality_adaptive_campaign::{
    QualityAdaptiveCampaign, QualityAdaptiveCampaignDisposition, QualityAdaptiveCampaignError,
    QualityAdaptiveCampaignRequest, QualityAdaptiveCampaignRound,
    QualityAdaptiveCampaignStopReason, execute_glioma_multimodal_quality_adaptive_campaign,
};
pub use quality_execution::{
    DryRunQualityScheduleExecutor, QualityExecutionApproval, QualityExecutionDisposition,
    QualityExecutionError, QualityExecutionFailure, QualityExecutionMode,
    QualityExecutionObservation, QualityExecutionRequest, QualityExecutionResult,
    QualityExecutionRun, QualityExecutionStopReason, QualityRunDisposition,
    QualityScheduleExecutor, execute_glioma_multimodal_quality_schedule,
};
pub use quality_recovery::{
    QualityRecoveryCampaignDisposition, QualityRecoveryError, QualityRecoveryModalityDisposition,
    QualityRecoveryModalityResult, QualityRecoveryObservation, QualityRecoveryPhase,
    QualityRecoveryRequest, QualityRecoveryResult, verify_glioma_multimodal_quality_recovery,
};
pub use quality_remediation::{
    QualityRemediationActionKind, QualityRemediationCandidate, QualityRemediationDisposition,
    QualityRemediationError, QualityRemediationPlan, QualityRemediationRequest,
    QualityRemediationStep, plan_glioma_multimodal_quality_remediation,
};
pub use quality_root_cause::{
    QualityIncidentSignal, QualityRootCause, QualityRootCauseAttribution,
    QualityRootCauseAttributionResult, QualityRootCauseCampaignDisposition,
    QualityRootCauseDisposition, QualityRootCauseError, QualityRootCauseRequest,
    QualitySignalScope, attribute_glioma_multimodal_quality_root_cause,
};
pub use quality_scheduler::{
    QualityAcquisitionCandidate, QualityScheduleAlternative, QualityScheduleDisposition,
    QualityScheduleError, QualityScheduleItem, QualitySchedulePlan, QualityScheduleRequest,
    plan_glioma_multimodal_quality_schedule,
};
pub use quality_transport::{
    QualityTransportCalibration, QualityTransportCell, QualityTransportDisposition,
    QualityTransportError, QualityTransportModalityDisposition, QualityTransportModalitySummary,
    QualityTransportRequest, calibrate_glioma_multimodal_quality_transport,
};
pub use readiness_gate::{
    MultimodalReadinessError, MultimodalReadinessRequest, MultimodalResearchReadiness,
    MultimodalResearchReadinessDisposition, MultimodalResearchSurface, MultimodalSurfaceDecision,
    MultimodalSurfaceReadiness, execute_glioma_multimodal_readiness_gate,
};
pub use reliability_calibration::{
    ReliabilityCalibration, ReliabilityCalibrationError, ReliabilityCalibrationRequest,
    ReliabilityDisposition, ReliabilityModalitySummary, ReliabilityObservation,
    calibrate_glioma_multimodal_reliability,
};
pub use sensitivity::{
    ModalitySensitivity, SensitivityAnalysis, SensitivityDisposition, SensitivityError,
    SensitivityRequest, analyze_glioma_multimodal_sensitivity,
};
pub use spatial_communication::{
    LigandReceptorPair, SpatialCommunicationAnalysis, SpatialCommunicationCell,
    SpatialCommunicationDisposition, SpatialCommunicationError, SpatialCommunicationPair,
    SpatialCommunicationPairDisposition, SpatialCommunicationRequest,
    analyze_glioma_spatial_communication,
};
pub use spatial_niche::{
    SpatialCell, SpatialNiche, SpatialNicheAnalysis, SpatialNicheDisposition, SpatialNicheError,
    SpatialNicheInteraction, SpatialNicheRequest, analyze_glioma_spatial_niches,
};
pub use spatial_propagation::{
    SpatialPropagationAnalysis, SpatialPropagationDisposition, SpatialPropagationEdge,
    SpatialPropagationError, SpatialPropagationRequest, SpatialPropagationTrajectory,
    analyze_glioma_spatial_state_propagation,
};
pub use spatial_registration::{
    RegisteredSpatialCell, RegistrationLandmark, SampleRegistration, SampleRegistrationDisposition,
    SpatialRegistrationAnalysis, SpatialRegistrationCell, SpatialRegistrationDisposition,
    SpatialRegistrationError, SpatialRegistrationRequest, register_glioma_spatial_samples,
};
pub use temporal_fusion::{
    TemporalFusionAnalysis, TemporalFusionDisposition, TemporalFusionError, TemporalFusionRequest,
    TemporalObservation, TemporalState, TemporalStateFeature, TemporalTransition,
    TemporalTransitionDirection, analyze_glioma_temporal_multimodal_fusion,
};
pub use temporal_spatial_alignment::{
    AlignedSampleState, AlignmentGate, SampleTimepoint, TemporalSpatialAction,
    TemporalSpatialAlignment, TemporalSpatialAlignmentError, TemporalSpatialAlignmentRequest,
    TemporalSpatialDisposition, analyze_glioma_temporal_spatial_alignment,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MultimodalIngestionQc;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P03")
}
