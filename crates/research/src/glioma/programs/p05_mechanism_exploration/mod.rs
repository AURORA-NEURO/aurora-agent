//! Mechanism exploration program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod discrimination;
pub mod graph_propagation;

pub use discrimination::{
    discriminate_mechanisms, MechanismDiscrimination, MechanismDiscriminationDisposition,
    MechanismDiscriminationError, MechanismDiscriminationRanking, MechanismDiscriminationRequest,
    MechanismDiscriminatorAction, MechanismFeatureObservation, MechanismHypothesis,
    MechanismInformationGain, MechanismPrediction,
};
pub use graph_propagation::{
    propagate_glioma_mechanism_graph, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismNodeScore,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MechanismExploration;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P05")
}
