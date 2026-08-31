//! Worldgen P31 local single-study contract model feature F05.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F05";const CONTRACT_VERSION:&str="worldgen-local-federated-commons-contract_model/1.0";
pub fn worldgen_local_federated_commons_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn admit_worldgen_local_federated_commons_contract(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}

