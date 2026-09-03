//! Section P32 multimodal multi-study research-copilot closure-integrity feature F07.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F07";const CONTRACT_VERSION:&str="section-multimodal-closure-integrity-research-copilot/1.0";
pub fn compile_section_multimodal_closure_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research-copilot")}
pub fn compile_section_multimodal_closure_integrity_research_copilot(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research-copilot")}
