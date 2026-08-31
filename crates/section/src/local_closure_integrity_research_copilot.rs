//! Section P32 local single-study research-copilot closure-integrity feature F03.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F03";const CONTRACT_VERSION:&str="section-local-closure-integrity-research-copilot/1.0";
pub fn compile_section_local_closure_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn compile_section_local_closure_integrity_research_copilot(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
