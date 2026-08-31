//! Worldgen P30 local single-study contract model feature F05.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F05";const CONTRACT_VERSION:&str="worldgen-local-adversarial-recovery-contract_model/1.0";
pub fn worldgen_local_adversarial_recovery_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn recover_worldgen_local_adversarial_recovery_contract(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}

