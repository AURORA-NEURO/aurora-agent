//! Worldgen P31 local single-study research copilot feature F09.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F09";const CONTRACT_VERSION:&str="worldgen-local-federated-commons-research_copilot/1.0";
pub fn worldgen_local_federated_commons_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn admit_worldgen_local_federated_commons_copilot(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}

