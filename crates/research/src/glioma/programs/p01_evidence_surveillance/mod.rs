//! Evidence surveillance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod priority;
pub mod surveillance;

pub use priority::{
    prioritize_glioma_evidence, EvidencePriorityAction, EvidencePriorityActionKind,
    EvidencePriorityDisposition, EvidencePriorityError, EvidencePriorityPlan,
    EvidencePriorityRequest, EvidencePriorityWeights,
};
pub use surveillance::{
    surveil_glioma_evidence, EvidenceChange, EvidenceChangeKind, EvidenceSurveillance,
    EvidenceSurveillanceAction, EvidenceSurveillanceActionKind, EvidenceSurveillanceDisposition,
    EvidenceSurveillanceError, EvidenceSurveillanceRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceSurveillance;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P01")
}
