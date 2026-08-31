//! Mechanism exploration program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod discrimination;

pub use discrimination::{
    discriminate_mechanisms, MechanismDiscrimination, MechanismDiscriminationDisposition,
    MechanismDiscriminationError, MechanismDiscriminationRanking, MechanismDiscriminationRequest,
    MechanismDiscriminatorAction, MechanismFeatureObservation, MechanismHypothesis,
    MechanismInformationGain, MechanismPrediction,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MechanismExploration;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P05")
}
