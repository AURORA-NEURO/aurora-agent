//! Worldgen P08 AFA-worldgen-P08-F04 mechanism exploration inference.
use super::mechanism_exploration_support::{self,MechanismQuestion,MechanismPortfolio};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-mechanism-exploration/1.0";
pub fn worldgen_federated_continual_mechanism_exploration_inference_manifest()->serde_json::Value{mechanism_exploration_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismQuestion1@1","federated continual autonomous","A1")}
pub fn explore_worldgen_federated_continual_mechanisms(request:&MechanismQuestion)->Result<MechanismPortfolio,mechanism_exploration_support::MechanismExplorationError>{mechanism_exploration_support::explore(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use mechanism_exploration_support::{MechanismCandidate,MechanismExplorationError,MechanismPortfolio as WorldgenFederatedContinualMechanismportfolioInference,MechanismQuestion as WorldgenFederatedContinualMechanismquestionInference};

