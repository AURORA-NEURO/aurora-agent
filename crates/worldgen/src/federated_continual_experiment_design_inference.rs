//! Worldgen P09 AFA-worldgen-P09-F04 experiment_design exploration inference.
use super::experiment_design_support::{self,ExperimentDesignQuestion,ExperimentDesignPortfolio};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-experiment_design-exploration/1.0";
pub fn worldgen_federated_continual_experiment_design_inference_manifest()->serde_json::Value{experiment_design_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignQuestion1@1","federated continual autonomous","A1")}
pub fn explore_worldgen_federated_continual_experiment_designs(request:&ExperimentDesignQuestion)->Result<ExperimentDesignPortfolio,experiment_design_support::ExperimentDesignError>{experiment_design_support::design(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use experiment_design_support::{ExperimentDesignCandidate,ExperimentDesignError,ExperimentDesignPortfolio as WorldgenFederatedContinualExperimentDesignportfolioInference,ExperimentDesignQuestion as WorldgenFederatedContinualExperimentDesignquestionInference};

