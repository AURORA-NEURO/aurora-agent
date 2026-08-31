//! Reproducible computation program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod robustness;

pub use robustness::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ReproducibleComputation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P09")
}
