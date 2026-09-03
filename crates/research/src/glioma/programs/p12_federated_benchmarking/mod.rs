//! Federated benchmarking and governance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod consensus;

pub use consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkContribution,
    FederatedBenchmarkDisposition, FederatedBenchmarkError, FederatedBenchmarkRequest,
    FederatedBenchmarkSite, FederatedBenchmarkSiteDisposition,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::FederatedBenchmarking;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P12")
}
