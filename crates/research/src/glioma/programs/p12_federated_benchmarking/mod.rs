//! Federated benchmarking and governance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod campaign;
pub mod consensus;
pub mod mechanism_transport;

pub use campaign::{
    execute_federated_benchmark_campaign, DryRunFederatedBenchmarkCampaignExecutor,
    FederatedBenchmarkAction, FederatedBenchmarkActionKind, FederatedBenchmarkCampaign,
    FederatedBenchmarkCampaignDisposition, FederatedBenchmarkCampaignError,
    FederatedBenchmarkCampaignExecutor, FederatedBenchmarkCampaignRequest,
    FederatedBenchmarkCampaignRound, FederatedBenchmarkCampaignStopReason,
    FederatedBenchmarkExecutionFailure,
};

pub use consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkContribution,
    FederatedBenchmarkDisposition, FederatedBenchmarkError, FederatedBenchmarkRequest,
    FederatedBenchmarkSite, FederatedBenchmarkSiteDisposition,
};

pub use mechanism_transport::{
    analyze_federated_mechanism_transport, FederatedMechanismContribution,
    FederatedMechanismSite, FederatedMechanismTransportAnalysis,
    FederatedMechanismTransportDisposition, FederatedMechanismTransportError,
    FederatedMechanismTransportRequest, FederatedModelCoverage,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::FederatedBenchmarking;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P12")
}
