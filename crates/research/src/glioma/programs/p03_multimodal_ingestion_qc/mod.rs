//! Multimodal ingestion and quality-control program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod campaign;
pub mod concordance;
pub mod consensus;
pub mod dropout_stress;
pub mod graph_fusion;
pub mod harmonization;
pub mod latent_factors;
pub mod missingness_audit;
pub mod operating_cycle;
pub mod readiness_gate;
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
pub use dropout_stress::{
    analyze_glioma_multimodal_dropout_stress, DropoutModalitySignal, DropoutScenario,
    DropoutScenarioDisposition, DropoutScenarioResult, DropoutStressAnalysis,
    DropoutStressDisposition, DropoutStressError, DropoutStressRequest,
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
pub use operating_cycle::{
    execute_glioma_multimodal_operating_cycle, execute_glioma_multimodal_operating_cycle_dry_run,
    GliomaMultimodalOperatingCycle, GliomaMultimodalOperatingCycleDisposition,
    GliomaMultimodalOperatingCycleError, GliomaMultimodalOperatingCycleRequest,
    MultimodalExecutionMode,
};
pub use readiness_gate::{
    execute_glioma_multimodal_readiness_gate, MultimodalReadinessError, MultimodalReadinessRequest,
    MultimodalResearchReadiness, MultimodalResearchReadinessDisposition, MultimodalResearchSurface,
    MultimodalSurfaceDecision, MultimodalSurfaceReadiness,
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
