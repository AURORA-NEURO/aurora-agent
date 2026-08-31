//! Infra P32 local research-copilot reliability-integrity feature.
use super::reliability_integrity_support::{manifest,qualify,ReliabilityIntegrityCard7,ReliabilityIntegrityError,ReliabilityIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-infra-P32-F03";pub const CONTRACT_VERSION:&str="infra-local_reliability_integrity_research_copilot/1.0";
pub fn local_reliability_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
pub fn qualify_local_reliability_integrity_research_copilot(request:&ReliabilityIntegrityRequest4)->Result<ReliabilityIntegrityCard7,ReliabilityIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research-copilot")}
