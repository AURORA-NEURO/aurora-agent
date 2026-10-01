//! Causal interpretation and replication program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod adaptive_campaign;
pub mod adaptive_execution;
pub mod adaptive_frontier;
pub mod autonomous_stage_bridge;
pub mod campaign;
pub mod causal_adjustment;
pub mod causal_contrast;
pub mod claim_adjudication;
pub mod clone_outcomes;
pub mod closure_interpretation;
pub mod computation_evidence_gate;
pub mod cross_model_claim_envelope;
pub mod cross_model_replication_frontier;
pub mod dynamic_policy;
pub mod longitudinal_transport;
pub mod mediation;
pub mod meta_analysis;
pub mod operating_cycle;
pub mod outcome_evidence_panel;
pub mod outcome_missingness_sensitivity;
pub mod outcome_record;
pub mod outcome_reporting_audit;
pub mod prospective_contradiction;
pub mod replication_closure_campaign;
pub mod replication_closure_execution;
pub mod replication_closure_frontier;
pub mod replication_concordance;
pub mod sensitivity;
pub mod state_transition;
pub mod synthesis;
pub mod trajectory;
pub mod transportability;
pub mod validation_replication_campaign;
pub mod validation_replication_gate;
pub use adaptive_campaign::{
    AdaptiveInterpretationCampaign,
    AdaptiveInterpretationCampaignDisposition,
    AdaptiveInterpretationCampaignError,
    AdaptiveInterpretationCampaignRequest,
    AdaptiveInterpretationCampaignRound,
    AdaptiveInterpretationCampaignStopReason,
    AdaptiveInterpretationPlanner,
    AdaptiveInterpretationPlanningFailure,
    DryRunAdaptiveInterpretationPlanner,
    execute_glioma_adaptive_interpretation_campaign,
    execute_glioma_adaptive_interpretation_campaign_dry_run,
    AdaptiveActionOutcomeSummary,
};
pub use adaptive_execution::{
    AdaptiveFrontierExecution,
    AdaptiveFrontierExecutionDisposition,
    AdaptiveFrontierExecutionError,
    AdaptiveFrontierExecutionRequest,
    dry_run_glioma_adaptive_frontier_executor,
    execute_glioma_adaptive_frontier,
};
pub use adaptive_frontier::{
    AdaptiveFrontierCandidate,
    AdaptiveFrontierDisposition,
    AdaptiveFrontierError,
    AdaptiveFrontierRequest,
    AdaptiveResearchFrontier,
    AdaptiveTarget,
    plan_glioma_adaptive_research_frontier,
};
pub use autonomous_stage_bridge::{
    dry_run_glioma_interpretation_stage_worker,
    dry_run_glioma_replication_stage_worker,
    GliomaInterpretationStageBridgeError,
    GliomaInterpretationStageBridgeReceipt,
    GliomaInterpretationStageWorker,
    GliomaReplicationStageBridgeError,
    GliomaReplicationStageBridgeReceipt,
    GliomaReplicationStageWorker,
    INTERPRETATION_OUTPUT_SCHEMA,
    INTERPRETATION_PARENT_FEATURE_ID,
    REPLICATION_OUTPUT_SCHEMA,
    REPLICATION_PARENT_FEATURE_ID,
};
pub use campaign::{
    DryRunGliomaReplicationCampaignExecutor,
    GliomaReplicationAction,
    GliomaReplicationActionKind,
    GliomaReplicationCampaign,
    GliomaReplicationCampaignDisposition,
    GliomaReplicationCampaignError,
    GliomaReplicationCampaignExecutor,
    GliomaReplicationCampaignObservation,
    GliomaReplicationCampaignRequest,
    GliomaReplicationCampaignRound,
    GliomaReplicationCampaignStopReason,
    GliomaReplicationExecutionFailure,
    execute_glioma_replication_campaign,
};
pub use causal_adjustment::{
    CausalStratumSummary,
    StratifiedCausalActionKind,
    StratifiedCausalAdjustment,
    StratifiedCausalDisposition,
    StratifiedCausalError,
    StratifiedCausalRequest,
    StratifiedObservation,
    analyze_stratified_causal_adjustment,
};
pub use causal_contrast::{
    CausalContrastAnalysis,
    CausalContrastDisposition,
    CausalContrastError,
    CausalContrastRequest,
    UnitContrast,
    analyze_glioma_causal_contrast,
};
pub use claim_adjudication::{
    CausalClaimDisposition,
    ClaimActionKind,
    ClaimGate,
    ClaimGateDisposition,
    ClaimNextAction,
    GliomaCausalClaimAdjudication,
    GliomaCausalClaimAdjudicationError,
    GliomaCausalClaimAdjudicationRequest,
    execute_glioma_causal_claim_adjudication,
};
pub use clone_outcomes::{
    ClonePanelBranchAnalysis,
    ClonePanelCandidateAnalysis,
    ClonePanelCellAnalysis,
    ClonePanelCellDisposition,
    ClonePanelMeasurementState,
    ClonePanelObservation,
    ClonePanelOutcomeAnalysis,
    ClonePanelOutcomeDisposition,
    ClonePanelOutcomeError,
    ClonePanelOutcomeRequest,
    analyze_glioma_clone_panel_outcomes,
};
pub use computation_evidence_gate::{
    ComputationInterpretationEvidenceGate,
    ComputationInterpretationEvidenceGateDisposition,
    ComputationInterpretationEvidenceGateError,
    ComputationInterpretationEvidenceGateRequest,
    ComputationInterpretationObservation,
    ComputationInterpretationSynthesisPolicy,
    execute_glioma_computation_interpretation_evidence_gate,
};
pub use cross_model_claim_envelope::{
    analyze_glioma_cross_model_claim_envelope,
    CrossModelClaimEnvelope,
    CrossModelClaimEnvelopeDisposition,
    CrossModelClaimEnvelopeError,
    CrossModelClaimEnvelopeRequest,
    CrossModelStudyEstimate,
    CrossModelSystemEnvelope,
};
pub use cross_model_replication_frontier::{
    materialize_glioma_cross_model_replication_actions,
    plan_glioma_cross_model_replication_frontier,
    CrossModelFollowUpKind,
    CrossModelReplicationCandidate,
    CrossModelReplicationFrontier,
    CrossModelReplicationFrontierDisposition,
    CrossModelReplicationFrontierError,
    CrossModelReplicationFrontierRequest,
    CrossModelReplicationScore,
};
pub use dynamic_policy::{
    DynamicPolicyCandidate,
    DynamicPolicyContribution,
    DynamicPolicyDisposition,
    DynamicPolicyError,
    DynamicPolicyEvaluation,
    DynamicPolicyObservation,
    DynamicPolicyRequest,
    DynamicPolicyRule,
    DynamicPolicyScore,
    DynamicPolicyScoreDisposition,
    DynamicPolicyTrajectory,
    evaluate_glioma_dynamic_policies,
};
pub use longitudinal_transport::{
    LongitudinalStudyExclusion, LongitudinalStudyTrend, LongitudinalTimepointEstimate,
    LongitudinalTransportAnalysis, LongitudinalTransportContribution,
    LongitudinalTransportDisposition, LongitudinalTransportError, LongitudinalTransportObservation,
    LongitudinalTransportRequest, analyze_glioma_longitudinal_transport,
};
pub use lineage_dynamics::{
    analyze_glioma_lineage_dynamics,
    ArmStateDynamics,
    BootstrapInterval,
    ComponentCall,
    LineageDynamicsAnalysis,
    LineageDynamicsDisposition,
    LineageDynamicsError,
    LineageDynamicsRequest,
    LineageStateCount,
    LineageStateSnapshot,
    LineageUnitDynamics,
    StateLineageDynamicsContrast,
    UnitEligibility,
    UnitStateDynamics,
};
pub use lineage_propagation::{
    analyze_glioma_lineage_propagation,
    CoefficientDisposition,
    EffectDisposition,
    HeldOutForecast,
    LineagePropagationAnalysis,
    LineagePropagationBatchSensitivity,
    LineagePropagationBatchSensitivityDisposition,
    LineagePropagationBootstrapDraw,
    LineagePropagationCoefficient,
    LineagePropagationContrast,
    LineagePropagationDisposition,
    LineagePropagationError,
    LineagePropagationOperator,
    LineagePropagationRequest,
    LineagePropagationSnapshot,
    PpmInterval,
    PropagationDestinationShare,
    SourceStatePropagation,
};
pub use lineage_response_decomposition::{
    analyze_glioma_lineage_response_decomposition,
    LineageResponseDecomposition,
    LineageResponseDecompositionDisposition,
    LineageResponseDecompositionError,
    LineageResponseDecompositionRequest,
    LineageResponseFollowUpFocus,
    LineageResponsePairDecomposition,
};
pub use lineage_transport::{
    analyze_glioma_lineage_transport,
    LineageTransportAnalysis,
    LineageTransportContrast,
    LineageTransportContrastDisposition,
    LineageTransportDisposition,
    LineageTransportError,
    LineageTransportFollowUpKind,
    LineageTransportRequest,
    LineageTransportStudy,
    LineageTransportStudyExclusion,
    LineageTransportStudyExclusionReason,
    LineageTransportSystemContrast,
};
pub use mediation::{
    MediationAnalysis,
    MediationDisposition,
    MediationError,
    MediationObservation,
    MediationRequest,
    analyze_glioma_mediation,
};
pub use prospective_contradiction::{
    CandidateAssessment, CandidateEligibility, ContradictionEvidence, EvidenceAssessment,
    HypothesisEvidenceSummary, ProspectiveContradictionDisposition, ProspectiveContradictionError,
    ProspectiveContradictionPlan, ProspectiveContradictionRequest, ResolverCandidate,
    ResolverPrediction, RivalHypothesis, RivalPair, RivalPairSeparation, SelectedResolver,
    analyze_glioma_prospective_contradiction,
};
pub use replication_concordance::{
    ExpectedEffectDirection, MultiStudyConcordance, MultiStudyConcordanceDisposition,
    MultiStudyConcordanceError, MultiStudyConcordanceRequest, MultiStudyEffect,
    MultiStudyEffectStatus, MultiStudyEffectSummary, MultiStudyExclusion, MultiStudyPair,
    MultiStudyPairRelation, MultiStudyPairwiseComparison, analyze_glioma_multistudy_concordance,
};
pub use closure_interpretation::{
    ClosureInterpretationDisposition,
    ClosureInterpretationError,
    ClosureInterpretationRequest,
    ClosureInterpretationRun,
    interpret_glioma_replication_closure,
};
pub use meta_analysis::{
    MetaAnalysisDisposition,
    MetaAnalysisError,
    MetaAnalysisRequest,
    MetaStudyContribution,
    ReplicationMetaAnalysis,
    analyze_replication_meta_analysis,
};
pub use operating_cycle::{
    GliomaInterpretationOperatingCycle,
    GliomaInterpretationOperatingCycleError,
    GliomaInterpretationOperatingCycleRequest,
    InterpretationOperatingCycleDisposition,
    execute_glioma_interpretation_operating_cycle,
};
pub use outcome_evidence_panel::{
    OutcomeAvailabilityCount, OutcomeEvidencePanelError, RegisteredOutcomeEvidencePanel,
    RegisteredOutcomeEvidencePanelRequest, build_glioma_registered_outcome_evidence_panel,
};
pub use outcome_missingness_sensitivity::{
    ObservedDirection, OutcomeAvailability, OutcomeMissingnessSensitivity,
    OutcomeSensitivityDirection, OutcomeSensitivityDisposition, OutcomeSensitivityError,
    OutcomeSensitivityRequest, OutcomeSensitivityRow, OutcomeSensitivityStudy,
    analyze_glioma_outcome_missingness_sensitivity,
};
pub use outcome_record::{
    OutcomeRecordError, RegisteredOutcomeRecord, build_glioma_registered_outcome_record,
};
pub use outcome_reporting_audit::{
    OutcomeAssessment, OutcomeAssessmentStatus, OutcomeReportingAuditError,
    OutcomeReportingAuditRequest, OutcomeReportingDisposition, RegisteredOutcome,
    RegisteredOutcomeReportingAudit, RegisteredOutcomeRole, RegisteredOutcomeStudy,
    RegisteredStudyStatus, RegistrationTiming, ReportedOutcome, ReportedOutcomeStatus,
    StudyOutcomeReport, StudyReportingAssessment, audit_glioma_registered_outcome_reporting,
};
pub use replication_assay_mapping_ledger::{
    compile_glioma_replication_assay_mapping_ledger,
    AssayMappingDisposition,
    AssayMappingLedger,
    AssayMappingLedgerError,
    AssayMappingLedgerRequest,
    AssayVariableSpec,
    CalibrationObservation,
    EquivalenceTier,
    MappingProposal,
    MappingTransform,
};
pub use replication_closure_campaign::{
    ReplicationClosureCampaignDisposition,
    ReplicationClosureCampaignError,
    ReplicationClosureCampaignRequest,
    ReplicationClosureCampaignRound,
    ReplicationClosureCampaignRun,
    ReplicationClosureCampaignStopReason,
    execute_glioma_replication_closure_campaign,
};
pub use replication_closure_execution::{
    ReplicationClosureExecutionDisposition,
    ReplicationClosureExecutionError,
    ReplicationClosureExecutionRequest,
    ReplicationClosureExecutionRun,
    execute_glioma_replication_closure,
};
pub use replication_closure_frontier::{
    ReplicationClosureCandidate,
    ReplicationClosureDisposition,
    ReplicationClosureFrontier,
    ReplicationClosureFrontierError,
    ReplicationClosureFrontierRequest,
    ReplicationClosureScore,
    ReplicationClosureTarget,
    plan_glioma_replication_closure_frontier,
};
pub use replication_protocol_schema::{
    compile_glioma_replication_protocol_schema,
    IndependentSiteSpec,
    MaskingSpec,
    PrespecifiedAnalysis,
    ProtocolDeviation,
    ProtocolUnknown,
    RandomizationSpec,
    RandomizationUnit,
    ReplicationEstimand,
    ReplicationProtocol,
    ReplicationProtocolDisposition,
    ReplicationProtocolError,
    ReplicationProtocolRequest,
    StoppingRule,
};
pub use sensitivity::{
    CausalSensitivityAnalysis,
    SensitivityDirection,
    SensitivityDisposition,
    SensitivityError,
    SensitivityObservation,
    SensitivityPoint,
    SensitivityRequest,
    analyze_causal_sensitivity,
};
pub use state_transition::{
    StateTransitionAnalysis,
    StateTransitionCell,
    StateTransitionContrast,
    StateTransitionDisposition,
    StateTransitionError,
    StateTransitionObservation,
    StateTransitionRequest,
    TransitionCellDisposition,
    TransitionContrastDisposition,
    TransitionDirection,
    analyze_glioma_state_transitions,
    StateTransitionBatchSensitivity,
    StateTransitionBatchSensitivityDisposition,
    MAX_BATCH_SENSITIVITY_BATCHES,
};
pub use synthesis::{
    InterpretationEvidence,
    InterpretationEvidenceDirection,
    InterpretationEvidenceFamily,
    InterpretationFamilySummary,
    InterpretationSynthesis,
    InterpretationSynthesisDisposition,
    InterpretationSynthesisError,
    InterpretationSynthesisRequest,
    synthesize_glioma_interpretation,
};
pub use trajectory::{
    TrajectoryAnalysis,
    TrajectoryArmSummary,
    TrajectoryDisposition,
    TrajectoryError,
    TrajectoryObservation,
    TrajectoryRequest,
    UnitTrajectory,
    UnitTrajectoryDisposition,
    analyze_glioma_trajectories,
};
pub use transportability::{
    TransportStudy,
    TransportStudyContribution,
    TransportabilityAnalysis,
    TransportabilityDisposition,
    TransportabilityError,
    TransportabilityRequest,
    analyze_glioma_transportability,
};
pub use validation_replication_campaign::{
    ValidationReplicationCampaignDisposition,
    ValidationReplicationCampaignError,
    ValidationReplicationCampaignRequest,
    ValidationReplicationCampaignRun,
    execute_glioma_validation_replication_campaign,
};
pub use validation_replication_gate::{
    ValidationReplicationGate,
    ValidationReplicationGateDisposition,
    ValidationReplicationGateError,
    ValidationReplicationGateRequest,
    plan_glioma_validation_replication_gate,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InterpretationReplication;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P10")
}

// Merged vertical feature modules.
pub mod lineage_dynamics;
pub mod lineage_propagation;
pub mod lineage_response_decomposition;
pub mod lineage_transport;
pub mod replication_assay_mapping_ledger;
pub mod replication_protocol_schema;
