//! Worldgen P31 multimodal multi-study inference feature F02.
use super::federated_commons_support::{admit,manifest,FederatedCommonsCard7,FederatedCommonsRequest4};
const FEATURE_ID:&str="AFA-worldgen-P31-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-federated-commons-inference/1.0";
pub fn worldgen_multimodal_federated_commons_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn admit_worldgen_multimodal_federated_commons(request:&FederatedCommonsRequest4)->Result<FederatedCommonsCard7,super::federated_commons_support::FederatedCommonsError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}

