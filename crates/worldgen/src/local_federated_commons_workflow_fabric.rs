//! Worldgen P31 local single-study workflow fabric feature F13.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F13";const CONTRACT_VERSION:&str="worldgen-local-federated-commons-workflow_fabric/1.0";
pub fn worldgen_local_federated_commons_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn admit_worldgen_local_federated_commons_workflow(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}

