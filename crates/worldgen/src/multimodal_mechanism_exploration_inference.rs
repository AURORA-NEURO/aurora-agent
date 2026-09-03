//! Worldgen P08 AFA-worldgen-P08-F02 mechanism exploration inference.
use super::mechanism_exploration_support::{self,MechanismQuestion,MechanismPortfolio};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-mechanism-exploration/1.0";
pub fn worldgen_multimodal_mechanism_exploration_inference_manifest()->serde_json::Value{mechanism_exploration_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismQuestion1@1","multimodal multi-study","A1")}
pub fn explore_worldgen_multimodal_mechanisms(request:&MechanismQuestion)->Result<MechanismPortfolio,mechanism_exploration_support::MechanismExplorationError>{mechanism_exploration_support::explore(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use mechanism_exploration_support::{MechanismCandidate,MechanismExplorationError,MechanismPortfolio as WorldgenMultimodalMechanismportfolioInference,MechanismQuestion as WorldgenMultimodalMechanismquestionInference};

