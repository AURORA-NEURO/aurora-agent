//! Federated benchmarking and governance program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod adaptive_campaign;
pub mod campaign;
pub mod consensus;
pub mod federated_interpretation;
pub mod mechanism_transport;
pub mod operating_cycle;
pub mod power;
pub mod replication_transport;
pub mod site_planner;
pub mod transport_campaign;

pub use adaptive_campaign::{
    FederatedBenchmarkAdaptiveCampaign, FederatedBenchmarkAdaptiveCampaignError,
    FederatedBenchmarkAdaptiveCampaignRequest, FederatedBenchmarkAdaptiveDisposition,
    FederatedBenchmarkAdaptiveStopReason, execute_federated_benchmark_adaptive_campaign,
    execute_federated_benchmark_adaptive_campaign_dry_run,
};
pub use campaign::{
    DryRunFederatedBenchmarkCampaignExecutor, FederatedBenchmarkAction,
    FederatedBenchmarkActionKind, FederatedBenchmarkCampaign,
    FederatedBenchmarkCampaignDisposition, FederatedBenchmarkCampaignError,
    FederatedBenchmarkCampaignExecutor, FederatedBenchmarkCampaignRequest,
    FederatedBenchmarkCampaignRound, FederatedBenchmarkCampaignStopReason,
    FederatedBenchmarkExecutionFailure, execute_federated_benchmark_campaign,
};

pub use consensus::{
    FederatedBenchmarkConsensus, FederatedBenchmarkContribution, FederatedBenchmarkDisposition,
    FederatedBenchmarkError, FederatedBenchmarkRequest, FederatedBenchmarkSite,
    FederatedBenchmarkSiteDisposition, analyze_federated_benchmark,
};
pub use federated_interpretation::{
    FederatedInterpretationDisposition, FederatedInterpretationError,
    FederatedInterpretationRequest, FederatedInterpretationRun, interpret_glioma_federated_closure,
};

pub use mechanism_transport::{
    FederatedMechanismContribution, FederatedMechanismSite, FederatedMechanismTransportAnalysis,
    FederatedMechanismTransportDisposition, FederatedMechanismTransportError,
    FederatedMechanismTransportRequest, FederatedModelCoverage,
    analyze_federated_mechanism_transport,
};
pub use power::{
    FederatedBenchmarkPower, FederatedBenchmarkPowerContribution,
    FederatedBenchmarkPowerDisposition, FederatedBenchmarkPowerError,
    FederatedBenchmarkPowerRequest, FederatedBenchmarkPowerSiteDisposition,
    analyze_federated_benchmark_power,
};

pub use transport_campaign::{
    DryRunFederatedMechanismTransportExecutor, FederatedMechanismTransportAction,
    FederatedMechanismTransportCampaign, FederatedMechanismTransportCampaignDisposition,
    FederatedMechanismTransportCampaignError, FederatedMechanismTransportCampaignRequest,
    FederatedMechanismTransportCampaignRound, FederatedMechanismTransportCampaignStopReason,
    FederatedMechanismTransportExecutionFailure, FederatedMechanismTransportExecutor,
    execute_federated_mechanism_transport_campaign,
    execute_federated_mechanism_transport_campaign_dry_run,
};

pub use site_planner::{
    FederatedBenchmarkCandidate, FederatedBenchmarkCandidateScore,
    FederatedBenchmarkPlanDisposition, FederatedBenchmarkSitePlan,
    FederatedBenchmarkSitePlannerError, FederatedBenchmarkSitePlannerRequest,
    plan_federated_benchmark_sites,
};

pub use operating_cycle::{
    FederatedBenchmarkExecutionMode, FederatedBenchmarkOperatingCycle,
    FederatedBenchmarkOperatingCycleDisposition, FederatedBenchmarkOperatingCycleError,
    FederatedBenchmarkOperatingCycleRequest, execute_federated_benchmark_operating_cycle,
    execute_federated_benchmark_operating_cycle_dry_run,
};

pub use replication_transport::{
    ReplicationAggregateQuality, ValidationReplicationTransportDisposition,
    ValidationReplicationTransportError, ValidationReplicationTransportRequest,
    ValidationReplicationTransportRun, execute_validation_replication_transport,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::FederatedBenchmarking;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P12")
}
