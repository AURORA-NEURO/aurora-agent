//! Section P32 multimodal multi-study inference closure-integrity feature F05.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F05";const CONTRACT_VERSION:&str="section-multimodal-closure-integrity-inference/1.0";
pub fn compile_section_multimodal_closure_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn compile_section_multimodal_closure_integrity_inference(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
