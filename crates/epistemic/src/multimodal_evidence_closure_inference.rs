//! Epistemic P32 multimodal multi-study inference evidence-closure feature F05.
use super::evidence_closure_support::{qualify,manifest,EvidenceClosureCard7,EvidenceClosureRequest4};
const FEATURE_ID:&str="AFA-epistemic-P32-F05";const CONTRACT_VERSION:&str="epistemic-multimodal-evidence-closure-inference/1.0";
pub fn epistemic_multimodal_evidence_closure_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn qualify_epistemic_multimodal_evidence_closure_inference(request:&EvidenceClosureRequest4)->Result<EvidenceClosureCard7,super::evidence_closure_support::EvidenceClosureError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
