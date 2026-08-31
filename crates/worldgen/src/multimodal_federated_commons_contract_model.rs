//! Worldgen P31 multimodal multi-study contract model feature F06.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F06";const CONTRACT_VERSION:&str="worldgen-multimodal-federated-commons-contract_model/1.0";
pub fn worldgen_multimodal_federated_commons_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
pub fn admit_worldgen_multimodal_federated_commons_contract(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}

