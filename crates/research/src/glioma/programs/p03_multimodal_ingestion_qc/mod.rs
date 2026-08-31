//! Multimodal ingestion and quality-control program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod concordance;
pub mod consensus;
pub mod harmonization;

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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MultimodalIngestionQc;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P03")
}
