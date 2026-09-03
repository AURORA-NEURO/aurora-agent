//! Worldgen P30 multimodal multi-study contract model feature F06.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F06";const CONTRACT_VERSION:&str="worldgen-multimodal-adversarial-recovery-contract_model/1.0";
pub fn worldgen_multimodal_adversarial_recovery_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
pub fn recover_worldgen_multimodal_adversarial_recovery_contract(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}

