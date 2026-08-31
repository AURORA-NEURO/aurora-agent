//! Adaptive P32 local single-study research-copilot posterior-integrity feature F03.
use super::posterior_integrity_support::{qualify,manifest,PosteriorIntegrityCard7,PosteriorIntegrityRequest4};
const FEATURE_ID:&str="AFA-adaptive-P32-F03";const CONTRACT_VERSION:&str="adaptive-local-posterior-integrity-research_copilot/1.0";
pub fn adaptive_local_posterior_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn qualify_adaptive_local_posterior_integrity_research_copilot(request:&PosteriorIntegrityRequest4)->Result<PosteriorIntegrityCard7,super::posterior_integrity_support::PosteriorIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
