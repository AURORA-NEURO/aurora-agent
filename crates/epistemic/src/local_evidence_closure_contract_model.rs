//! Epistemic P32 local single-study contract-model evidence-closure feature F02.
use super::evidence_closure_support::{qualify,manifest,EvidenceClosureCard7,EvidenceClosureRequest4};
const FEATURE_ID:&str="AFA-epistemic-P32-F02";const CONTRACT_VERSION:&str="epistemic-local-evidence-closure-contract-model/1.0";
pub fn epistemic_local_evidence_closure_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn qualify_epistemic_local_evidence_closure_contract_model(request:&EvidenceClosureRequest4)->Result<EvidenceClosureCard7,super::evidence_closure_support::EvidenceClosureError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
