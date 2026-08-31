//! Worldgen P31 multimodal multi-study research copilot feature F10.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F10";const CONTRACT_VERSION:&str="worldgen-multimodal-federated-commons-research_copilot/1.0";
pub fn worldgen_multimodal_federated_commons_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
pub fn admit_worldgen_multimodal_federated_commons_copilot(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}

