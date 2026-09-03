//! Conformance P32 multimodal multi-study research_copilot replay-integrity feature F07.
use super::replay_integrity_support::{qualify,manifest,ReplayIntegrityCard7,ReplayIntegrityRequest4,ReplayIntegrityError};
const FEATURE_ID:&str="AFA-conformance-P32-F07";const CONTRACT_VERSION:&str="conformance-multimodal-replay-integrity-research_copilot/1.0";
pub fn conformance_multimodal_replay_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research_copilot")}
pub fn qualify_conformance_multimodal_replay_integrity_research_copilot(request:&ReplayIntegrityRequest4)->Result<ReplayIntegrityCard7,ReplayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research_copilot")}
