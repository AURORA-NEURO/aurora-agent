//! Section P32 prospective high-throughput contract-model closure-integrity feature F10.
use super::closure_integrity_support::{compile,manifest,ClosureIntegrityCard7,ClosureIntegrityRequest4};
const FEATURE_ID:&str="AFA-section-P32-F10";const CONTRACT_VERSION:&str="section-throughput-closure-integrity-contract-model/1.0";
pub fn compile_section_throughput_closure_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract-model")}
pub fn compile_section_throughput_closure_integrity_contract_model(request:&ClosureIntegrityRequest4)->Result<ClosureIntegrityCard7,super::closure_integrity_support::ClosureIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract-model")}
