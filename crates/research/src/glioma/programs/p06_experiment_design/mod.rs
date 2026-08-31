//! Power-aware experiment-design program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod dose_response;

pub use dose_response::{
    analyze_glioma_dose_response, DoseDirection, DoseResponseAnalysis, DoseResponseDisposition,
    DoseResponseError, DoseResponseObservation, DoseResponsePoint, DoseResponseRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ExperimentDesign;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P06")
}
