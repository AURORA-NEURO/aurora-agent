//! Federated benchmarking and governance program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod adaptive_campaign;
pub mod aggregate_anomaly_detector;
pub mod aggregate_phenotype_schema;
pub mod autonomous_stage_bridge;
pub mod benchmark_director;
pub mod benchmark_execution_record;
pub mod benchmark_job_scheduler;
pub mod campaign;
pub mod capacity_planner;
pub mod consensus;
pub mod continual_benchmark_monitor;
pub mod contribution_integrity;
pub mod cross_site_evidence_explorer;
pub mod dry_run_coordinator;
pub mod federated_interpretation;
pub mod federation_operations;
pub mod governance_cycle;
pub mod heterogeneity_adaptive_power;
pub mod mechanism_transport;
pub mod multisite_benchmark_workflow;
pub mod operating_cycle;
pub mod participant_api;
pub mod power;
pub mod quorum_admission;
pub mod replication_transport;
pub mod signed_aggregate_api;
pub mod site_capability_envelope;
pub mod site_participation;
pub mod site_planner;
pub mod site_provenance_attestation;
pub mod site_selection_agent;
pub mod small_consortium_bias;
pub mod transport_campaign;

pub use adaptive_campaign::{
    FederatedBenchmarkAdaptiveCampaign, FederatedBenchmarkAdaptiveCampaignError,
    FederatedBenchmarkAdaptiveCampaignRequest, FederatedBenchmarkAdaptiveDisposition,
    FederatedBenchmarkAdaptiveStopReason, execute_federated_benchmark_adaptive_campaign,
    execute_federated_benchmark_adaptive_campaign_dry_run,
};
pub use aggregate_anomaly_detector::{
    analyze_federated_aggregate_anomalies, AggregateAnomalyDisposition, AggregateAnomalyKind,
    AggregateAnomalyRecord, FederatedAggregateAnomalyAssessment, FederatedAggregateAnomalyError,
    FederatedAggregateAnomalyRequest, FederatedAggregateObservation,
};
pub use aggregate_phenotype_schema::{
    compile_glioma_aggregate_phenotype_summary, AggregatePhenotypeDisposition,
    AggregatePhenotypeError, AggregatePhenotypePolicy, AggregatePhenotypeRequest,
    AggregatePhenotypeSummary, LocalPhenotypeField, PhenotypeMappingKind,
};
pub use autonomous_stage_bridge::{
    dry_run_glioma_federation_stage_worker, GliomaFederationStageBridgeError,
    GliomaFederationStageBridgeReceipt, GliomaFederationStageWorker,
};
pub use benchmark_director::{
    build_glioma_benchmark_director_snapshot, BenchmarkDirectorAlert, BenchmarkDirectorAlertKind,
    BenchmarkDirectorDisposition, BenchmarkDirectorError, BenchmarkDirectorProposal,
    BenchmarkDirectorProposalKind, BenchmarkDirectorRequest, BenchmarkDirectorRunInput,
    BenchmarkDirectorRunStatus, BenchmarkDirectorRunView, BenchmarkDirectorSnapshot,
};
pub use benchmark_execution_record::{
    execute_glioma_federated_benchmark_record, FederatedBenchmarkExecutionError,
    FederatedBenchmarkExecutionRequest, FederatedBenchmarkRunRecord, FederatedBenchmarkRunStatus,
    FederatedBenchmarkSiteContribution,
};
pub use benchmark_job_scheduler::{
    execute_glioma_benchmark_job, BenchmarkJobDisposition, BenchmarkJobError, BenchmarkJobEvent,
    BenchmarkJobEventKind, BenchmarkJobRequest, BenchmarkJobSiteInput, BenchmarkJobSiteStage,
    BenchmarkJobSiteState, BenchmarkJobStage, FederatedBenchmarkJob,
};
pub use campaign::{
    DryRunFederatedBenchmarkCampaignExecutor, FederatedBenchmarkAction,
    FederatedBenchmarkActionKind, FederatedBenchmarkCampaign,
    FederatedBenchmarkCampaignDisposition, FederatedBenchmarkCampaignError,
    FederatedBenchmarkCampaignExecutor, FederatedBenchmarkCampaignRequest,
    FederatedBenchmarkCampaignRound, FederatedBenchmarkCampaignStopReason,
    FederatedBenchmarkExecutionFailure, execute_federated_benchmark_campaign,
};
pub use cross_site_evidence_explorer::{
    explore_glioma_cross_site_evidence, CrossSiteEvidenceExplorerError,
    CrossSiteEvidenceExplorerRequest, CrossSiteEvidenceObservation, CrossSiteEvidenceView,
    EvidenceCell, EvidenceCellDisposition,
};

pub use capacity_planner::{
    plan_federation_capacity, CapacityForecastWindow, CapacityPlanDisposition, CapacitySiteWindow,
    FederationCapacityError, FederationCapacityPlan, FederationCapacityRequest,
};
pub use consensus::{
    FederatedBenchmarkConsensus, FederatedBenchmarkContribution, FederatedBenchmarkDisposition,
    FederatedBenchmarkError, FederatedBenchmarkRequest, FederatedBenchmarkSite,
    FederatedBenchmarkSiteDisposition, analyze_federated_benchmark,
};
pub use continual_benchmark_monitor::{
    monitor_glioma_federated_benchmark_continuity, BenchmarkCalibrationState,
    ContinualBenchmarkAssessment, ContinualBenchmarkDisposition, ContinualBenchmarkError,
    ContinualBenchmarkMonitorRequest, ContinualWindowSummary, FederatedBenchmarkWindow,
};
pub use contribution_integrity::{
    verify_glioma_contribution_integrity, ContributionIntegrityError, ContributionIntegrityReason,
    ContributionIntegrityRecord, ContributionIntegrityRequest, ContributionIntegrityReview,
    ContributionIntegrityStatus, FederatedContributionIntegrity,
};
pub use dry_run_coordinator::{
    execute_federated_benchmark_dry_run, DryRunCheck, DryRunCheckStatus, DryRunDisposition,
    FederatedBenchmarkDryRunReport, FederatedBenchmarkDryRunRequest, FederatedDryRunError,
    FederatedDryRunSiteFixture,
};
pub use federated_interpretation::{
    FederatedInterpretationDisposition, FederatedInterpretationError,
    FederatedInterpretationRequest, FederatedInterpretationRun, interpret_glioma_federated_closure,
};
pub use federation_operations::{
    build_glioma_federation_operations_snapshot, FederationOperationalFindingKind,
    FederationOperationsError, FederationOperationsRequest, FederationOperationsSnapshot,
    FederationSiteOperationalState, FederationSiteOperationsInput, FederationSiteOperationsView,
};
pub use governance_cycle::{
    compile_glioma_benchmark_governance_cycle, GovernanceCycleError, GovernanceCycleRecord,
    GovernanceCycleRequest, GovernanceCycleStatus, GovernanceStage, GovernanceTransition,
    GovernanceTransitionDecision, GovernanceVote, GovernanceVoteDecision,
};
pub use participant_api::{
    execute_glioma_participant_exchange, FederationParticipantExchange,
    FederationParticipantReceipt, FederationParticipantRequest, ParticipantApiError,
    ParticipantExchangeAction, ParticipantExchangeStatus,
};
pub use site_selection_agent::{
    plan_federated_glioma_sites, FederatedSiteSelectionError, FederatedSiteSelectionPlan,
    FederatedSiteSelectionRequest, GliomaSiteCapabilityEnvelope, SiteSelectionDisposition,
    SiteSelectionScore,
};
pub use small_consortium_bias::{
    assess_federated_benchmark_small_consortium_bias, FederatedBenchmarkBiasAssessment,
    FederatedBenchmarkBiasCorrectionRequest, FederatedBenchmarkBiasDisposition,
    FederatedBenchmarkBiasError, FederatedBenchmarkBiasSensitivityDisposition,
    FederatedBenchmarkBiasSensitivityPoint,
};

pub use heterogeneity_adaptive_power::{
    plan_glioma_heterogeneity_adaptive_benchmark_power, FederatedPowerSiteEnvelope,
    HeterogeneityAdaptivePowerError, HeterogeneityAdaptivePowerPlan,
    HeterogeneityAdaptivePowerRequest, PowerRecommendation, PowerSensitivityPoint,
};
pub use mechanism_transport::{
    FederatedMechanismContribution, FederatedMechanismSite, FederatedMechanismTransportAnalysis,
    FederatedMechanismTransportDisposition, FederatedMechanismTransportError,
    FederatedMechanismTransportRequest, FederatedModelCoverage,
    analyze_federated_mechanism_transport,
};
pub use multisite_benchmark_workflow::{
    execute_glioma_multisite_benchmark_workflow, FederatedBenchmarkSitePolicyResponse,
    FederatedBenchmarkWorkflowBudget, FederatedBenchmarkWorkflowDisposition,
    FederatedBenchmarkWorkflowError, FederatedBenchmarkWorkflowEvent,
    FederatedBenchmarkWorkflowEventKind, FederatedBenchmarkWorkflowRequest,
    FederatedBenchmarkWorkflowRun, FederatedBenchmarkWorkflowSiteInput,
    FederatedBenchmarkWorkflowSiteState, FederatedBenchmarkWorkflowStageRecord,
    FederatedWorkflowStage, FederatedWorkflowStageState,
};
pub use power::{
    FederatedBenchmarkPower, FederatedBenchmarkPowerContribution,
    FederatedBenchmarkPowerDisposition, FederatedBenchmarkPowerError,
    FederatedBenchmarkPowerRequest, FederatedBenchmarkPowerSiteDisposition,
    analyze_federated_benchmark_power,
};
pub use quorum_admission::{
    assess_glioma_quorum_admission, FederatedQuorumContribution, QuorumAdmissionDecision,
    QuorumAdmissionDisposition, QuorumAdmissionError, QuorumAdmissionRequest,
    QuorumContributionReview, QuorumContributionStatus, QuorumExclusionReason,
};
pub use signed_aggregate_api::{
    submit_glioma_signed_aggregate, AggregateSubmissionReason, AggregateSubmissionReceipt,
    AggregateSubmissionStatus, SignedAggregateApiError, SignedAggregateContribution,
    SignedAggregateSubmissionRequest,
};
pub use site_capability_envelope::{
    compile_glioma_site_capability_envelope, SiteCapabilityDisposition, SiteCapabilityEnvelope,
    SiteCapabilityError, SiteCapabilityPolicy, SiteCapabilityRecord, SiteCapabilityRequest,
};
pub use site_participation::{
    review_glioma_site_participation, ParticipationDisposition, SiteParticipationError,
    SiteParticipationRequest, SiteParticipationReview,
};
pub use site_provenance_attestation::{
    attest_glioma_site_contribution, SiteAttestationStatus, SiteContributionAttestation,
    SitePolicyDecision, SiteProvenanceAttestationError, SiteProvenanceAttestationRequest,
    SiteSignerChain,
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
