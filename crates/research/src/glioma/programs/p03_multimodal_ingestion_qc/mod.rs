//! Multimodal ingestion and quality-control program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod concordance;
pub mod consensus;
pub mod harmonization;
pub mod latent_factors;
pub mod spatial_communication;
pub mod spatial_niche;

pub use concordance::{
    analyze_multimodal_concordance, ConcordanceDisposition, ConcordanceError, ConcordanceRequest,
    FeatureValue, ModalityConcordance, ModalityVector, MultimodalConcordance,
    PairConcordanceDisposition,
};
pub use consensus::{
    analyze_multimodal_consensus, ConsensusAssignment, ConsensusCluster, ConsensusDisposition,
    ConsensusError, ConsensusRequest, MultimodalConsensus,
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
pub use spatial_communication::{
    analyze_glioma_spatial_communication, LigandReceptorPair, SpatialCommunicationAnalysis,
    SpatialCommunicationCell, SpatialCommunicationDisposition, SpatialCommunicationError,
    SpatialCommunicationPair, SpatialCommunicationPairDisposition, SpatialCommunicationRequest,
};
pub use spatial_niche::{
    analyze_glioma_spatial_niches, SpatialCell, SpatialNiche, SpatialNicheAnalysis,
    SpatialNicheDisposition, SpatialNicheError, SpatialNicheInteraction, SpatialNicheRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MultimodalIngestionQc;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P03")
}
