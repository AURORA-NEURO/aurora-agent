//! Mechanism exploration program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod counterfactual;
pub mod discrimination;
pub mod ensemble_counterfactual;
pub mod graph_propagation;
pub mod robust_portfolio;

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
