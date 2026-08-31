//! Worldgen P30 local single-study workflow fabric feature F13.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F13";const CONTRACT_VERSION:&str="worldgen-local-adversarial-recovery-workflow_fabric/1.0";
pub fn worldgen_local_adversarial_recovery_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn recover_worldgen_local_adversarial_recovery_workflow(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}

