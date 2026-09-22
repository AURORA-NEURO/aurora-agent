//! Federated benchmarking and governance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

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
    execute_federated_benchmark_adaptive_campaign,
    execute_federated_benchmark_adaptive_campaign_dry_run, FederatedBenchmarkAdaptiveCampaign,
    FederatedBenchmarkAdaptiveCampaignError, FederatedBenchmarkAdaptiveCampaignRequest,
    FederatedBenchmarkAdaptiveDisposition, FederatedBenchmarkAdaptiveStopReason,
};
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
pub use federated_interpretation::{
    interpret_glioma_federated_closure, FederatedInterpretationDisposition,
    FederatedInterpretationError, FederatedInterpretationRequest, FederatedInterpretationRun,
};

pub use mechanism_transport::{
    analyze_federated_mechanism_transport, FederatedMechanismContribution, FederatedMechanismSite,
    FederatedMechanismTransportAnalysis, FederatedMechanismTransportDisposition,
    FederatedMechanismTransportError, FederatedMechanismTransportRequest, FederatedModelCoverage,
};
pub use power::{
    analyze_federated_benchmark_power, FederatedBenchmarkPower,
    FederatedBenchmarkPowerContribution, FederatedBenchmarkPowerDisposition,
    FederatedBenchmarkPowerError, FederatedBenchmarkPowerRequest,
    FederatedBenchmarkPowerSiteDisposition,
};

pub use transport_campaign::{
    execute_federated_mechanism_transport_campaign,
    execute_federated_mechanism_transport_campaign_dry_run,
    DryRunFederatedMechanismTransportExecutor, FederatedMechanismTransportAction,
    FederatedMechanismTransportCampaign, FederatedMechanismTransportCampaignDisposition,
    FederatedMechanismTransportCampaignError, FederatedMechanismTransportCampaignRequest,
    FederatedMechanismTransportCampaignRound, FederatedMechanismTransportCampaignStopReason,
    FederatedMechanismTransportExecutionFailure, FederatedMechanismTransportExecutor,
};

pub use site_planner::{
    plan_federated_benchmark_sites, FederatedBenchmarkCandidate, FederatedBenchmarkCandidateScore,
    FederatedBenchmarkPlanDisposition, FederatedBenchmarkSitePlan,
    FederatedBenchmarkSitePlannerError, FederatedBenchmarkSitePlannerRequest,
};

pub use operating_cycle::{
    execute_federated_benchmark_operating_cycle,
    execute_federated_benchmark_operating_cycle_dry_run, FederatedBenchmarkExecutionMode,
    FederatedBenchmarkOperatingCycle, FederatedBenchmarkOperatingCycleDisposition,
    FederatedBenchmarkOperatingCycleError, FederatedBenchmarkOperatingCycleRequest,
};

pub use replication_transport::{
    execute_validation_replication_transport, ReplicationAggregateQuality,
    ValidationReplicationTransportDisposition, ValidationReplicationTransportError,
    ValidationReplicationTransportRequest, ValidationReplicationTransportRun,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::FederatedBenchmarking;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P12")
}
