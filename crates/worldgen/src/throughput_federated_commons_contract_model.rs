//! Worldgen P31 prospective high-throughput contract model feature F07.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F07";const CONTRACT_VERSION:&str="worldgen-throughput-federated-commons-contract_model/1.0";
pub fn worldgen_throughput_federated_commons_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}
pub fn admit_worldgen_throughput_federated_commons_contract(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}

