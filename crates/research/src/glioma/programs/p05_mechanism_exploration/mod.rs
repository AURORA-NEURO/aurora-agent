//! Mechanism exploration program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod action_planner;
pub mod adaptive_policy;
pub mod bayesian_update;
pub mod calibrated_campaign;
pub mod calibration;
pub mod clonal_evolution;
pub mod closed_loop;
pub mod consensus;
pub mod counterfactual;
pub mod discrimination;
pub mod discrimination_campaign;
pub mod ensemble_counterfactual;
pub mod evidence_assimilation;
pub mod feedback_replan;
pub mod fidelity_bridge;
pub mod graph_propagation;
pub mod identifiability;
pub mod intervention_value;
pub mod invariance;
pub mod mechanism_dynamics;
pub mod mechanism_workflow;
pub mod multi_fidelity_control;
pub mod multi_study_workflow;
pub mod operating_cycle;
pub mod pathway_activity;
pub mod prospective_controller;
pub mod robust_portfolio;
pub mod robustness_stress;
pub mod state_filter;
pub mod state_smoother;
pub mod workflow_assurance;

pub use action_planner::{
    GliomaMechanismActionPlanner, MechanismActionPlan, MechanismActionPlannerConfig,
    MechanismActionPlannerError, compile_mechanism_action_plan,
};
pub use adaptive_policy::{
    AdaptiveMechanismAction, AdaptiveMechanismActionScore, AdaptiveMechanismCampaign,
    AdaptiveMechanismCampaignRequest, AdaptiveMechanismCampaignRound,
    AdaptiveMechanismCampaignStopReason, AdaptiveMechanismExecutionFailure, AdaptiveMechanismModel,
    AdaptiveMechanismObservation, AdaptiveMechanismPolicy, AdaptiveMechanismPolicyDisposition,
    AdaptiveMechanismPolicyError, AdaptiveMechanismPolicyExecutor,
    AdaptiveMechanismPolicyPosterior, AdaptiveMechanismPolicyRequest, AdaptiveMechanismPolicyStep,
    AdaptiveMechanismPrediction, DryRunAdaptiveMechanismPolicyExecutor,
    execute_glioma_adaptive_mechanism_campaign, plan_glioma_adaptive_mechanism_policy,
};
pub use bayesian_update::{
    BayesianMechanismHypothesis, BayesianMechanismUpdateError, BayesianMechanismUpdateRequest,
    BayesianUpdateDisposition, MechanismBayesianUpdate, MechanismPosteriorRecord,
    MechanismPosteriorStatus, update_glioma_mechanism_posterior,
};
pub use calibrated_campaign::{
    CalibratedMechanismActionScore, CalibratedMechanismCampaign,
    CalibratedMechanismCampaignDisposition, CalibratedMechanismCampaignError,
    CalibratedMechanismCampaignRequest, CalibratedMechanismCampaignRound,
    CalibratedMechanismCampaignStopReason, CalibratedMechanismTrust,
    execute_glioma_calibrated_mechanism_campaign,
    execute_glioma_calibrated_mechanism_campaign_dry_run,
};
pub use calibration::{
    MechanismCalibration, MechanismCalibrationBin, MechanismCalibrationError,
    MechanismCalibrationObservation, MechanismCalibrationRequest, MechanismCalibrationScore,
    MechanismCalibrationScoreDisposition, calibrate_glioma_mechanisms,
};
pub use clonal_evolution::{
    ClonalEdge, ClonalEvolutionDisposition, ClonalEvolutionError, ClonalEvolutionGraph,
    ClonalEvolutionRequest, ClonalNode, ClonalRelation, CloneMarker, CloneMarkerState,
    CloneProfile, analyze_glioma_clonal_evolution,
};
pub use closed_loop::{
    MechanismClosedLoopActionScore, MechanismClosedLoopCandidate, MechanismClosedLoopDecision,
    MechanismClosedLoopDisposition, MechanismClosedLoopError, MechanismClosedLoopPlan,
    MechanismClosedLoopRequest, plan_glioma_mechanism_closed_loop,
};
pub use consensus::{
    MechanismConsensus, MechanismConsensusDisposition, MechanismConsensusError,
    MechanismConsensusRecord, MechanismConsensusRequest, MechanismEvidencePacket,
    MechanismSourceAgreement, compile_glioma_mechanism_consensus,
};
pub use counterfactual::{
    CounterfactualContrast, CounterfactualDirection, CounterfactualDisposition,
    CounterfactualError, CounterfactualIntervention, CounterfactualRequest,
    MechanismCounterfactual, simulate_glioma_counterfactual,
};
pub use discrimination::{
    MechanismDiscrimination, MechanismDiscriminationDisposition, MechanismDiscriminationError,
    MechanismDiscriminationRanking, MechanismDiscriminationRequest, MechanismDiscriminatorAction,
    MechanismFeatureObservation, MechanismHypothesis, MechanismInformationGain,
    MechanismPrediction, discriminate_mechanisms,
};
pub use discrimination_campaign::{
    DryRunMechanismDiscriminationCampaignExecutor, MechanismDiscriminationCampaign,
    MechanismDiscriminationCampaignDisposition, MechanismDiscriminationCampaignError,
    MechanismDiscriminationCampaignExecutionFailure, MechanismDiscriminationCampaignExecutor,
    MechanismDiscriminationCampaignRequest, MechanismDiscriminationCampaignRound,
    MechanismDiscriminationCampaignStopReason, execute_glioma_mechanism_discrimination_campaign,
};
pub use ensemble_counterfactual::{
    CounterfactualEnsembleRequest, CounterfactualModel, EnsembleCounterfactualError,
    EnsembleDirection, EnsembleDisposition, EnsembleModelResult, EnsembleTargetSummary,
    MechanismCounterfactualEnsemble, simulate_glioma_counterfactual_ensemble,
};
pub use evidence_assimilation::{
    AssimilatedMechanismRecord, AssimilatedMechanismStatus, MechanismEvidenceAssimilation,
    MechanismEvidenceAssimilationDisposition, MechanismEvidenceAssimilationError,
    MechanismEvidenceAssimilationRequest, MechanismEvidenceSnapshot,
    assimilate_glioma_mechanism_evidence,
};
pub use feedback_replan::{
    MechanismFeedbackActionScore, MechanismFeedbackCandidate, MechanismFeedbackDecision,
    MechanismFeedbackObservation, MechanismFeedbackOutcome, MechanismFeedbackReplan,
    MechanismFeedbackReplanDisposition, MechanismFeedbackReplanError,
    MechanismFeedbackReplanRequest, replan_glioma_mechanism_feedback,
};
pub use fidelity_bridge::{
    MechanismFidelityBridge, MechanismFidelityBridgeDisposition, MechanismFidelityBridgeError,
    MechanismFidelityBridgeRequest, MechanismFidelityObservation, MechanismFidelityRecord,
    MechanismFidelityResidual, bridge_glioma_mechanism_fidelity,
};
pub use graph_propagation::{
    MechanismGraphDisposition, MechanismGraphEdge, MechanismGraphError, MechanismGraphNode,
    MechanismGraphPropagation, MechanismGraphRelation, MechanismGraphRequest, MechanismNodeScore,
    propagate_glioma_mechanism_graph,
};
pub use identifiability::{
    IdentifiabilityFeature, IdentifiabilityFeatureUtility, IdentifiabilityMechanism,
    MechanismIdentifiability, MechanismIdentifiabilityDisposition, MechanismIdentifiabilityError,
    MechanismIdentifiabilityRequest, MechanismPairIdentifiability,
    analyze_glioma_mechanism_identifiability,
};
pub use intervention_value::{
    MechanismInterventionCandidate, MechanismInterventionPrediction, MechanismInterventionValue,
    MechanismInterventionValueDisposition, MechanismInterventionValueError,
    MechanismInterventionValueRequest, MechanismInterventionValueScore,
    analyze_glioma_mechanism_intervention_value,
};
pub use invariance::{
    InvarianceMechanism, MechanismInvariance, MechanismInvarianceContext,
    MechanismInvarianceDisposition, MechanismInvarianceError, MechanismInvariancePair,
    MechanismInvarianceRequest, MechanismInvarianceSignatureScore, MechanismInvarianceUtility,
    MechanismSignature, analyze_glioma_mechanism_invariance,
};
pub use mechanism_dynamics::{
    MechanismDynamicsDisposition, MechanismDynamicsEdge, MechanismDynamicsError,
    MechanismDynamicsIntervention, MechanismDynamicsNode, MechanismDynamicsPlan,
    MechanismDynamicsRequest, MechanismDynamicsSensitivity, MechanismDynamicsState,
    MechanismDynamicsStep, simulate_glioma_mechanism_dynamics,
};
pub use mechanism_workflow::{
    MechanismWorkflowAction, MechanismWorkflowDisposition, MechanismWorkflowError,
    MechanismWorkflowNode, MechanismWorkflowNodeStatus, MechanismWorkflowPlan,
    MechanismWorkflowRequest, compile_glioma_mechanism_workflow,
};
pub use multi_fidelity_control::{
    MultiFidelityControlActionScore, MultiFidelityControlCandidate, MultiFidelityControlDecision,
    MultiFidelityControlDisposition, MultiFidelityControlError, MultiFidelityControlPlan,
    MultiFidelityControlRequest, plan_glioma_mechanism_multi_fidelity_control,
};
pub use multi_study_workflow::{
    MechanismFederationMode, MechanismMultiStudyActionGroup, MechanismMultiStudyTask,
    MechanismMultiStudyTaskStatus, MechanismMultiStudyWorkflowDisposition,
    MechanismMultiStudyWorkflowError, MechanismMultiStudyWorkflowPlan,
    MechanismMultiStudyWorkflowRequest, MechanismStudyLane,
    compile_glioma_multi_study_mechanism_workflow,
};
pub use operating_cycle::{
    MechanismOperatingCycle, MechanismOperatingCycleDisposition, MechanismOperatingCycleError,
    MechanismOperatingCycleRequest, execute_glioma_mechanism_operating_cycle,
};
pub use pathway_activity::{
    PathwayActivityAnalysis, PathwayActivityDefinition, PathwayActivityDirection,
    PathwayActivityDisposition, PathwayActivityEdge, PathwayActivityError, PathwayActivityNode,
    PathwayActivityObservation, PathwayActivityRecord, PathwayActivityRequest,
    analyze_glioma_pathway_activity,
};
pub use prospective_controller::{
    MechanismProspectiveControllerError, MechanismProspectiveControllerPlan,
    MechanismProspectiveControllerRequest, MechanismProspectiveDisposition,
    MechanismProspectiveObservation, MechanismProspectiveOutcome,
    MechanismProspectiveResourceCapacity, MechanismProspectiveResourceUsage,
    MechanismProspectiveTaskDecision, MechanismProspectiveTaskPolicy,
    MechanismProspectiveTaskStatus, control_glioma_mechanism_prospective_batch,
};
pub use robust_portfolio::{
    PortfolioDirection, RobustInterventionCandidate, RobustInterventionPortfolio,
    RobustInterventionRequest, RobustInterventionScore, RobustPortfolioDisposition,
    RobustPortfolioError, plan_glioma_robust_intervention_portfolio,
};
pub use robustness_stress::{
    MechanismRobustnessEvaluation, MechanismRobustnessRecord, MechanismRobustnessStress,
    MechanismRobustnessStressDisposition, MechanismRobustnessStressError,
    MechanismRobustnessStressRequest, MechanismStressAdjustment, MechanismStressCandidate,
    MechanismStressScenario, MechanismStressScenarioScore, stress_glioma_mechanism_robustness,
};
pub use state_filter::{
    MechanismStateFilterDisposition, MechanismStateFilterError, MechanismStateFilterRequest,
    MechanismStateFilterResult, MechanismStateModel, MechanismStateObservation,
    MechanismStatePosterior, filter_glioma_mechanism_states,
};
pub use state_smoother::{
    MechanismStateSmoothPosterior, MechanismStateSmootherDisposition, MechanismStateSmootherError,
    MechanismStateSmootherRequest, MechanismStateSmootherResult, MechanismStateTransitionSupport,
    smooth_glioma_mechanism_states,
};
pub use workflow_assurance::{
    MechanismAssuranceDecision, MechanismAssuranceDisposition, MechanismAssuranceStatus,
    MechanismSafetyEvidenceState, MechanismSafetyObservation, MechanismWorkflowAssuranceError,
    MechanismWorkflowAssurancePlan, MechanismWorkflowAssurancePolicy,
    MechanismWorkflowAssuranceRequest, assure_glioma_mechanism_workflow,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MechanismExploration;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P05")
}
