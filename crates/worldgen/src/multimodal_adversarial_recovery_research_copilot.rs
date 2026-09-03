//! Worldgen P30 multimodal multi-study research copilot feature F10.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F10";const CONTRACT_VERSION:&str="worldgen-multimodal-adversarial-recovery-research_copilot/1.0";
pub fn worldgen_multimodal_adversarial_recovery_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
pub fn recover_worldgen_multimodal_adversarial_recovery_copilot(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}

