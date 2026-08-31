//! Worldgen P30 multimodal multi-study workflow fabric feature F14.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F14";const CONTRACT_VERSION:&str="worldgen-multimodal-adversarial-recovery-workflow_fabric/1.0";
pub fn worldgen_multimodal_adversarial_recovery_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}
pub fn recover_worldgen_multimodal_adversarial_recovery_workflow(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}

