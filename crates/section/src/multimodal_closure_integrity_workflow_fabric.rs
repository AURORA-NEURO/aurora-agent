//! Section P32 multimodal multi-study workflow-fabric closure-integrity feature F08.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F08";const CONTRACT_VERSION:&str="section-multimodal-closure-integrity-workflow-fabric/1.0";
pub fn compile_section_multimodal_closure_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow-fabric")}
pub fn compile_section_multimodal_closure_integrity_workflow_fabric(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow-fabric")}
