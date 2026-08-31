//! Section P32 local single-study inference closure-integrity feature F01.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F01";const CONTRACT_VERSION:&str="section-local-closure-integrity-inference/1.0";
pub fn compile_section_local_closure_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn compile_section_local_closure_integrity_inference(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
