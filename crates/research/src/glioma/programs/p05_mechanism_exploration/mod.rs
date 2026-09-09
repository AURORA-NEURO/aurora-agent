//! Mechanism exploration program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod action_planner;
pub mod adaptive_policy;
pub mod calibration;
pub mod clonal_evolution;
pub mod counterfactual;
pub mod discrimination;
pub mod discrimination_campaign;
pub mod ensemble_counterfactual;
pub mod graph_propagation;
pub mod pathway_activity;
pub mod robust_portfolio;

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
pub use clonal_evolution::{
    analyze_glioma_clonal_evolution, CloneMarker, CloneMarkerState, CloneProfile, ClonalEdge,
    ClonalEvolutionDisposition, ClonalEvolutionError, ClonalEvolutionGraph,
    ClonalEvolutionRequest, ClonalNode, ClonalRelation,
};
pub use calibration::{
    calibrate_glioma_mechanisms, MechanismCalibration, MechanismCalibrationBin,
    MechanismCalibrationError, MechanismCalibrationObservation, MechanismCalibrationRequest,
    MechanismCalibrationScore, MechanismCalibrationScoreDisposition,
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
    execute_glioma_mechanism_discrimination_campaign, DryRunMechanismDiscriminationCampaignExecutor,
    MechanismDiscriminationCampaign, MechanismDiscriminationCampaignDisposition,
    MechanismDiscriminationCampaignError, MechanismDiscriminationCampaignExecutionFailure,
    MechanismDiscriminationCampaignExecutor, MechanismDiscriminationCampaignRequest,
    MechanismDiscriminationCampaignRound, MechanismDiscriminationCampaignStopReason,
};
pub use ensemble_counterfactual::{
    simulate_glioma_counterfactual_ensemble, CounterfactualEnsembleRequest, CounterfactualModel,
    EnsembleCounterfactualError, EnsembleDirection, EnsembleDisposition, EnsembleModelResult,
    EnsembleTargetSummary, MechanismCounterfactualEnsemble,
};
pub use graph_propagation::{
    propagate_glioma_mechanism_graph, MechanismGraphDisposition, MechanismGraphEdge,
    MechanismGraphError, MechanismGraphNode, MechanismGraphPropagation, MechanismGraphRelation,
    MechanismGraphRequest, MechanismNodeScore,
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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::MechanismExploration;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P05")
}
