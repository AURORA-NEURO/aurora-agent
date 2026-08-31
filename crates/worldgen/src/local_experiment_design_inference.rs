//! Worldgen P09 AFA-worldgen-P09-F01 experiment_design exploration inference.
use super::experiment_design_support::{self,ExperimentDesignQuestion,ExperimentDesignPortfolio};
pub const FEATURE_ID:&str="AFA-worldgen-P09-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-experiment_design-exploration/1.0";
pub fn worldgen_local_experiment_design_inference_manifest()->serde_json::Value{experiment_design_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ExperimentDesignQuestion1@1","local single-study","A0")}
pub fn explore_worldgen_local_experiment_designs(request:&ExperimentDesignQuestion)->Result<ExperimentDesignPortfolio,experiment_design_support::ExperimentDesignError>{experiment_design_support::design(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use experiment_design_support::{ExperimentDesignCandidate,ExperimentDesignError,ExperimentDesignPortfolio as WorldgenLocalExperimentDesignportfolioInference,ExperimentDesignQuestion as WorldgenLocalExperimentDesignquestionInference};

