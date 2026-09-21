//! Mechanism exploration program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod action_planner;
pub mod adaptive_policy;
pub mod bayesian_update;
pub mod calibrated_campaign;
pub mod calibration;
pub mod clonal_evolution;
pub mod consensus;
pub mod counterfactual;
pub mod discrimination;
pub mod discrimination_campaign;
pub mod ensemble_counterfactual;
pub mod evidence_assimilation;
pub mod fidelity_bridge;
pub mod graph_propagation;
pub mod identifiability;
pub mod intervention_value;
pub mod invariance;
pub mod mechanism_dynamics;
pub mod operating_cycle;
pub mod pathway_activity;
pub mod robust_portfolio;
pub mod robustness_stress;
pub mod state_filter;
pub mod state_smoother;

pub use action_planner::{
    compile_mechanism_action_plan, GliomaMechanismActionPlanner, MechanismActionPlan,
    MechanismActionPlannerConfig, MechanismActionPlannerError,
};
pub use adaptive_policy::{
    execute_glioma_adaptive_mechanism_campaign, plan_glioma_adaptive_mechanism_policy,
    AdaptiveMechanismAction, AdaptiveMechanismActionScore, AdaptiveMechanismCampaign,
    AdaptiveMechanismCampaignRequest, AdaptiveMechanismCampaignRound,
    AdaptiveMechanismCampaignStopReason, AdaptiveMechanismExecutionFailure, AdaptiveMechanismModel,
    AdaptiveMechanismObservation, AdaptiveMechanismPolicy, AdaptiveMechanismPolicyDisposition,
    AdaptiveMechanismPolicyError, AdaptiveMechanismPolicyExecutor,
    AdaptiveMechanismPolicyPosterior, AdaptiveMechanismPolicyRequest, AdaptiveMechanismPolicyStep,
    AdaptiveMechanismPrediction, DryRunAdaptiveMechanismPolicyExecutor,
};
pub use bayesian_update::{
    update_glioma_mechanism_posterior, BayesianMechanismHypothesis, BayesianMechanismUpdateError,
    BayesianMechanismUpdateRequest, BayesianUpdateDisposition, MechanismBayesianUpdate,
    MechanismPosteriorRecord, MechanismPosteriorStatus,
};
pub use calibrated_campaign::{
    execute_glioma_calibrated_mechanism_campaign,
    execute_glioma_calibrated_mechanism_campaign_dry_run, CalibratedMechanismActionScore,
    CalibratedMechanismCampaign, CalibratedMechanismCampaignDisposition,
    CalibratedMechanismCampaignError, CalibratedMechanismCampaignRequest,
    CalibratedMechanismCampaignRound, CalibratedMechanismCampaignStopReason,
    CalibratedMechanismTrust,
};
pub use calibration::{
    calibrate_glioma_mechanisms, MechanismCalibration, MechanismCalibrationBin,
    MechanismCalibrationError, MechanismCalibrationObservation, MechanismCalibrationRequest,
    MechanismCalibrationScore, MechanismCalibrationScoreDisposition,
};
pub use clonal_evolution::{
    analyze_glioma_clonal_evolution, ClonalEdge, ClonalEvolutionDisposition, ClonalEvolutionError,
    ClonalEvolutionGraph, ClonalEvolutionRequest, ClonalNode, ClonalRelation, CloneMarker,
    CloneMarkerState, CloneProfile,
};
pub use consensus::{
    compile_glioma_mechanism_consensus, MechanismConsensus, MechanismConsensusDisposition,
    MechanismConsensusError, MechanismConsensusRecord, MechanismConsensusRequest,
    MechanismEvidencePacket, MechanismSourceAgreement,
};
pub use counterfactual::{
    simulate_glioma_counterfactual, CounterfactualContrast, CounterfactualDirection,
    CounterfactualDisposition, CounterfactualError, CounterfactualIntervention,
    CounterfactualRequest, MechanismCounterfactual,
};
pub use discrimination::{
    discriminate_mechanisms, MechanismDiscrimination, MechanismDiscriminationDisposition,
    MechanismDiscriminationError, MechanismDiscriminationRanking, MechanismDiscriminationRequest,
    MechanismDiscriminatorAction, MechanismFeatureObservation, MechanismHypothesis,
    MechanismInformationGain, MechanismPrediction,
};
pub use discrimination_campaign::{
    execute_glioma_mechanism_discrimination_campaign,
    DryRunMechanismDiscriminationCampaignExecutor, MechanismDiscriminationCampaign,
    MechanismDiscriminationCampaignDisposition, MechanismDiscriminationCampaignError,
    MechanismDiscriminationCampaignExecutionFailure, MechanismDiscriminationCampaignExecutor,
    MechanismDiscriminationCampaignRequest, MechanismDiscriminationCampaignRound,
    MechanismDiscriminationCampaignStopReason,
};
pub use ensemble_counterfactual::{
    simulate_glioma_counterfactual_ensemble, CounterfactualEnsembleRequest, CounterfactualModel,
    EnsembleCounterfactualError, EnsembleDirection, EnsembleDisposition, EnsembleModelResult,
    EnsembleTargetSummary, MechanismCounterfactualEnsemble,
};
pub use evidence_assimilation::{
    assimilate_glioma_mechanism_evidence, AssimilatedMechanismRecord, AssimilatedMechanismStatus,
    MechanismEvidenceAssimilation, MechanismEvidenceAssimilationDisposition,
    MechanismEvidenceAssimilationError, MechanismEvidenceAssimilationRequest,
    MechanismEvidenceSnapshot,
};
pub use fidelity_bridge::{
    bridge_glioma_mechanism_fidelity, MechanismFidelityBridge, MechanismFidelityBridgeDisposition,
    MechanismFidelityBridgeError, MechanismFidelityBridgeRequest, MechanismFidelityObservation,
    MechanismFidelityRecord, MechanismFidelityResidual,
};
pub use graph_propagation::{
    propagate_glioma_mechanism_graph, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismNodeScore,
};
pub use identifiability::{
    analyze_glioma_mechanism_identifiability, IdentifiabilityFeature,
    IdentifiabilityFeatureUtility, IdentifiabilityMechanism, MechanismIdentifiability,
    MechanismIdentifiabilityDisposition, MechanismIdentifiabilityError,
    MechanismIdentifiabilityRequest, MechanismPairIdentifiability,
};
pub use intervention_value::{
    analyze_glioma_mechanism_intervention_value, MechanismInterventionCandidate,
    MechanismInterventionPrediction, MechanismInterventionValue,
    MechanismInterventionValueDisposition, MechanismInterventionValueError,
    MechanismInterventionValueRequest, MechanismInterventionValueScore,
};
pub use invariance::{
    analyze_glioma_mechanism_invariance, InvarianceMechanism, MechanismInvariance,
    MechanismInvarianceContext, MechanismInvarianceDisposition, MechanismInvarianceError,
    MechanismInvariancePair, MechanismInvarianceRequest, MechanismInvarianceSignatureScore,
    MechanismInvarianceUtility, MechanismSignature,
};
pub use mechanism_dynamics::{
    simulate_glioma_mechanism_dynamics, MechanismDynamicsDisposition, MechanismDynamicsEdge,
    MechanismDynamicsError, MechanismDynamicsIntervention, MechanismDynamicsNode,
    MechanismDynamicsPlan, MechanismDynamicsRequest, MechanismDynamicsSensitivity,
    MechanismDynamicsState, MechanismDynamicsStep,
};
pub use operating_cycle::{
    execute_glioma_mechanism_operating_cycle, MechanismOperatingCycle,
    MechanismOperatingCycleDisposition, MechanismOperatingCycleError,
    MechanismOperatingCycleRequest,
};
pub use pathway_activity::{
    analyze_glioma_pathway_activity, PathwayActivityAnalysis, PathwayActivityDefinition,
    PathwayActivityDirection, PathwayActivityDisposition, PathwayActivityEdge,
    PathwayActivityError, PathwayActivityNode, PathwayActivityObservation, PathwayActivityRecord,
    PathwayActivityRequest,
};
pub use robust_portfolio::{
    plan_glioma_robust_intervention_portfolio, PortfolioDirection, RobustInterventionCandidate,
    RobustInterventionPortfolio, RobustInterventionRequest, RobustInterventionScore,
    RobustPortfolioDisposition, RobustPortfolioError,
};
pub use robustness_stress::{
    stress_glioma_mechanism_robustness, MechanismRobustnessRecord, MechanismRobustnessStress,
    MechanismRobustnessStressDisposition, MechanismRobustnessStressError,
    MechanismRobustnessStressRequest, MechanismStressAdjustment, MechanismStressCandidate,
    MechanismStressScenario, MechanismStressScenarioScore,
};
pub use state_filter::{
    filter_glioma_mechanism_states, MechanismStateFilterDisposition, MechanismStateFilterError,
    MechanismStateFilterRequest, MechanismStateFilterResult, MechanismStateModel,
    MechanismStateObservation, MechanismStatePosterior,
};
pub use state_smoother::{
    smooth_glioma_mechanism_states, MechanismStateSmoothPosterior,
    MechanismStateSmootherDisposition, MechanismStateSmootherError, MechanismStateSmootherRequest,
    MechanismStateSmootherResult, MechanismStateTransitionSupport,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MechanismExploration;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P05")
}
