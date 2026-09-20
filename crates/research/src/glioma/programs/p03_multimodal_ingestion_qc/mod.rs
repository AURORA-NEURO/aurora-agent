//! Multimodal ingestion and quality-control program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

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
pub mod latent_factors;
pub mod missingness_audit;
pub mod modality_portfolio;
pub mod operating_cycle;
pub mod prospective_quality;
pub mod quality_adaptive_campaign;
pub mod quality_execution;
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
    execute_glioma_multimodal_ingestion_campaign, DryRunMultimodalIngestionCampaignExecutor,
    IngestionQcAction, IngestionQcActionKind, MultimodalIngestionCampaign,
    MultimodalIngestionCampaignDisposition, MultimodalIngestionCampaignError,
    MultimodalIngestionCampaignExecutor, MultimodalIngestionCampaignRequest,
    MultimodalIngestionCampaignRound, MultimodalIngestionCampaignStopReason,
    MultimodalIngestionExecutionFailure,
};
pub use concordance::{
    analyze_multimodal_concordance, ConcordanceDisposition, ConcordanceError, ConcordanceRequest,
    FeatureValue, ModalityConcordance, ModalityVector, MultimodalConcordance,
    PairConcordanceDisposition,
};
pub use consensus::{
    analyze_multimodal_consensus, ConsensusAssignment, ConsensusCluster, ConsensusDisposition,
    ConsensusError, ConsensusRequest, MultimodalConsensus,
};
pub use contradiction_adjudication::{
    adjudicate_glioma_multimodal_contradictions, ContradictionAdjudication,
    ContradictionAdjudicationDisposition, ContradictionAdjudicationError,
    ContradictionAdjudicationRequest, ContradictionKind, ModalityPairAdjudication,
};
pub use decision_gate::{
    analyze_glioma_multimodal_decision_gate, DecisionDirection, DecisionGateModalityObservation,
    MultimodalDecisionGateAnalysis, MultimodalDecisionGateDisposition, MultimodalDecisionGateError,
    MultimodalDecisionGateRequest,
};
pub use drift_surveillance::{
    surveil_glioma_multimodal_drift, DriftDisposition, DriftMetricSummary, DriftObservation,
    DriftSurveillance, DriftSurveillanceError, DriftSurveillanceRequest,
};
pub use dropout_stress::{
    analyze_glioma_multimodal_dropout_stress, DropoutModalitySignal, DropoutScenario,
    DropoutScenarioDisposition, DropoutScenarioResult, DropoutStressAnalysis,
    DropoutStressDisposition, DropoutStressError, DropoutStressRequest,
};
pub use evidence_fusion::{
    analyze_glioma_multimodal_evidence_fusion, EndpointEvidence, EvidenceContribution,
    EvidenceFusionAnalysis, EvidenceFusionDisposition, EvidenceFusionError, EvidenceFusionRequest,
};
pub use graph_fusion::{
    analyze_glioma_multimodal_graph_fusion, GraphFusionAnalysis, GraphFusionDisposition,
    GraphFusionError, GraphFusionNeighbour, GraphFusionRequest, GraphFusionState,
    GraphFusionVector,
};
pub use harmonization::{
    harmonize_glioma_multimodal_batches, BatchHarmonizationDiagnostic, HarmonizationDisposition,
    HarmonizationError, HarmonizationRequest, HarmonizationVector, HarmonizedFeature,
    HarmonizedVector, MultimodalHarmonization,
};
pub use latent_factors::{
    analyze_glioma_latent_factors, LatentFactorAnalysis, LatentFactorComponent,
    LatentFactorDisposition, LatentFactorError, LatentFactorRequest, LatentFactorVector,
    LatentLoading, LatentScore,
};
pub use missingness_audit::{
    analyze_glioma_multimodal_missingness, MissingnessAudit, MissingnessAuditDisposition,
    MissingnessAuditError, MissingnessAuditRequest, MissingnessModalitySummary,
    MissingnessObservation, MissingnessPairSummary, MissingnessPattern,
    MissingnessPatternDisposition, MissingnessState,
};
pub use modality_portfolio::{
    plan_glioma_multimodal_portfolio, ModalityCapability, ModalityPortfolioAlternative,
    ModalityPortfolioDisposition, ModalityPortfolioError, ModalityPortfolioPlan,
    ModalityPortfolioRequest,
};
pub use operating_cycle::{
    execute_glioma_multimodal_operating_cycle, execute_glioma_multimodal_operating_cycle_dry_run,
    GliomaMultimodalOperatingCycle, GliomaMultimodalOperatingCycleDisposition,
    GliomaMultimodalOperatingCycleError, GliomaMultimodalOperatingCycleRequest,
    MultimodalExecutionMode,
};
pub use prospective_quality::{
    forecast_glioma_multimodal_quality, ModalityQualityForecast, ProspectiveQualityError,
    ProspectiveQualityForecast, ProspectiveQualityRequest, QualityForecastDisposition,
    QualityForecastObservation,
};
pub use quality_adaptive_campaign::{
    execute_glioma_multimodal_quality_adaptive_campaign, QualityAdaptiveCampaign,
    QualityAdaptiveCampaignDisposition, QualityAdaptiveCampaignError,
    QualityAdaptiveCampaignRequest, QualityAdaptiveCampaignRound,
    QualityAdaptiveCampaignStopReason,
};
pub use quality_execution::{
    execute_glioma_multimodal_quality_schedule, DryRunQualityScheduleExecutor,
    QualityExecutionApproval, QualityExecutionDisposition, QualityExecutionError,
    QualityExecutionFailure, QualityExecutionMode, QualityExecutionObservation,
    QualityExecutionRequest, QualityExecutionResult, QualityExecutionRun,
    QualityExecutionStopReason, QualityRunDisposition, QualityScheduleExecutor,
};
pub use quality_scheduler::{
    plan_glioma_multimodal_quality_schedule, QualityAcquisitionCandidate,
    QualityScheduleAlternative, QualityScheduleDisposition, QualityScheduleError,
    QualityScheduleItem, QualitySchedulePlan, QualityScheduleRequest,
};
pub use quality_transport::{
    calibrate_glioma_multimodal_quality_transport, QualityTransportCalibration,
    QualityTransportCell, QualityTransportDisposition, QualityTransportError,
    QualityTransportModalityDisposition, QualityTransportModalitySummary, QualityTransportRequest,
};
pub use readiness_gate::{
    execute_glioma_multimodal_readiness_gate, MultimodalReadinessError, MultimodalReadinessRequest,
    MultimodalResearchReadiness, MultimodalResearchReadinessDisposition, MultimodalResearchSurface,
    MultimodalSurfaceDecision, MultimodalSurfaceReadiness,
};
pub use reliability_calibration::{
    calibrate_glioma_multimodal_reliability, ReliabilityCalibration, ReliabilityCalibrationError,
    ReliabilityCalibrationRequest, ReliabilityDisposition, ReliabilityModalitySummary,
    ReliabilityObservation,
};
pub use sensitivity::{
    analyze_glioma_multimodal_sensitivity, ModalitySensitivity, SensitivityAnalysis,
    SensitivityDisposition, SensitivityError, SensitivityRequest,
};
pub use spatial_communication::{
    analyze_glioma_spatial_communication, LigandReceptorPair, SpatialCommunicationAnalysis,
    SpatialCommunicationCell, SpatialCommunicationDisposition, SpatialCommunicationError,
    SpatialCommunicationPair, SpatialCommunicationPairDisposition, SpatialCommunicationRequest,
};
pub use spatial_niche::{
    analyze_glioma_spatial_niches, SpatialCell, SpatialNiche, SpatialNicheAnalysis,
    SpatialNicheDisposition, SpatialNicheError, SpatialNicheInteraction, SpatialNicheRequest,
};
pub use spatial_propagation::{
    analyze_glioma_spatial_state_propagation, SpatialPropagationAnalysis,
    SpatialPropagationDisposition, SpatialPropagationEdge, SpatialPropagationError,
    SpatialPropagationRequest, SpatialPropagationTrajectory,
};
pub use spatial_registration::{
    register_glioma_spatial_samples, RegisteredSpatialCell, RegistrationLandmark,
    SampleRegistration, SampleRegistrationDisposition, SpatialRegistrationAnalysis,
    SpatialRegistrationCell, SpatialRegistrationDisposition, SpatialRegistrationError,
    SpatialRegistrationRequest,
};
pub use temporal_fusion::{
    analyze_glioma_temporal_multimodal_fusion, TemporalFusionAnalysis, TemporalFusionDisposition,
    TemporalFusionError, TemporalFusionRequest, TemporalObservation, TemporalState,
    TemporalStateFeature, TemporalTransition, TemporalTransitionDirection,
};
pub use temporal_spatial_alignment::{
    analyze_glioma_temporal_spatial_alignment, AlignedSampleState, AlignmentGate, SampleTimepoint,
    TemporalSpatialAction, TemporalSpatialAlignment, TemporalSpatialAlignmentError,
    TemporalSpatialAlignmentRequest, TemporalSpatialDisposition,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MultimodalIngestionQc;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P03")
}
