//! Worldgen P30 prospective high-throughput research copilot feature F11.
use super::adversarial_recovery_support::{recover,manifest,AdversarialRecoveryCard7,AdversarialRecoveryRequest4};
const FEATURE_ID:&str="AFA-worldgen-P30-F11";const CONTRACT_VERSION:&str="worldgen-throughput-adversarial-recovery-research_copilot/1.0";
pub fn worldgen_throughput_adversarial_recovery_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}
pub fn recover_worldgen_throughput_adversarial_recovery_copilot(request:&AdversarialRecoveryRequest4)->Result<AdversarialRecoveryCard7,super::adversarial_recovery_support::AdversarialRecoveryError>{recover(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}

